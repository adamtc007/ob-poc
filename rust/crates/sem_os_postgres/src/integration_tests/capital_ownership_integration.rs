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
            let prefix = format!("captest_{}", &Uuid::new_v4().to_string()[..8]);
            Ok(Self { pool, prefix })
        }

        fn name(&self, base: &str) -> String {
            format!("{}_{}", self.prefix, base)
        }

        async fn cleanup(&self) -> Result<()> {
            let pattern = format!("{}%", self.prefix);

            // Clean up capital structure tables
            sqlx::query(
                r#"DELETE FROM "ob-poc".special_rights WHERE issuer_entity_id IN
                   (SELECT entity_id FROM "ob-poc".entities WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".dilution_instruments WHERE issuer_entity_id IN
                   (SELECT entity_id FROM "ob-poc".entities WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".holdings WHERE share_class_id IN
                   (SELECT id FROM "ob-poc".share_classes WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".issuance_events WHERE share_class_id IN
                   (SELECT id FROM "ob-poc".share_classes WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".share_class_supply WHERE share_class_id IN
                   (SELECT id FROM "ob-poc".share_classes WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".share_class_identifiers WHERE share_class_id IN
                   (SELECT id FROM "ob-poc".share_classes WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(r#"DELETE FROM "ob-poc".share_classes WHERE name LIKE $1"#)
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
            INSERT INTO "ob-poc".share_classes (
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
            r#"SELECT name, instrument_kind, votes_per_unit FROM "ob-poc".share_classes WHERE id = $1"#,
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
            INSERT INTO "ob-poc".share_classes (
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
            INSERT INTO "ob-poc".share_classes (
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
            r#"SELECT votes_per_unit FROM "ob-poc".share_classes WHERE id = $1"#,
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
            INSERT INTO "ob-poc".share_classes (
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
            INSERT INTO "ob-poc".share_class_supply (
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
            FROM "ob-poc".share_class_supply
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
            INSERT INTO "ob-poc".share_classes (
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
            INSERT INTO "ob-poc".share_class_supply (share_class_id, as_of_date, issued_units, outstanding_units)
            VALUES ($1, CURRENT_DATE, 1000, 1000)
            "#,
            share_class_id
        )
        .execute(&db.pool)
        .await?;

        // Create holding - holder owns 600 shares (60%)
        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".holdings (share_class_id, investor_entity_id, units, status)
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
            FROM "ob-poc".holdings
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
            INSERT INTO "ob-poc".share_classes (
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
            INSERT INTO "ob-poc".special_rights (
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
            FROM "ob-poc".special_rights
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
            INSERT INTO "ob-poc".share_classes (
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
            INSERT INTO "ob-poc".dilution_instruments (
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
            FROM "ob-poc".dilution_instruments
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
            INSERT INTO "ob-poc".share_classes (
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
            INSERT INTO "ob-poc".share_class_identifiers (share_class_id, scheme_code, identifier_value, is_primary)
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
            FROM "ob-poc".share_class_identifiers
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
            INSERT INTO "ob-poc".share_classes (
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
            INSERT INTO "ob-poc".issuance_events (
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
            INSERT INTO "ob-poc".issuance_events (
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
            FROM "ob-poc".issuance_events
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

    // =========================================================================
    // CREATE -> ISSUE.INITIAL -> ISSUE.NEW (EOP-PLAN capital/share-class
    // consolidation, 2026-08-19)
    //
    // Exercises the fixed path end-to-end through the real SemOsVerbOp
    // dispatch layer (not raw SQL, unlike the tests above): the share class
    // is created with the 4 fields `share-class.create` gained
    // (issuer_entity_id/instrument_kind/votes_per_unit/economic_per_unit —
    // same columns `capital.share-class.create` used to write before it was
    // deleted for referencing the nonexistent `authorized_shares` column),
    // then `IssueInitial`/`IssueNew` run for real, proving IssueInitial no
    // longer errors on its removed dead-column UPDATE and that both events
    // fold correctly into share_class_supply.
    // =========================================================================

    struct RollbackScope {
        id: ob_poc_types::TransactionScopeId,
        tx: sqlx::Transaction<'static, sqlx::Postgres>,
        pool: PgPool,
    }

    impl dsl_runtime::TransactionScope for RollbackScope {
        fn scope_id(&self) -> ob_poc_types::TransactionScopeId {
            self.id
        }
        fn transaction(&mut self) -> &mut sqlx::Transaction<'static, sqlx::Postgres> {
            &mut self.tx
        }
        fn pool(&self) -> &PgPool {
            &self.pool
        }
    }

    #[tokio::test]
    async fn test_create_issue_initial_issue_new_via_ops() -> Result<()> {
        use crate::ops::capital::{IssueInitial, IssueNew};
        use crate::ops::SemOsVerbOp;
        use dsl_runtime::{TransactionScope, VerbExecutionContext, VerbExecutionOutcome};
        use sem_os_core::principal::Principal;

        let db = TestDb::new().await?;
        let issuer_name = db.name("consolidation_issuer");
        let class_name = db.name("consolidation_class");

        let cbu_id = db.get_or_create_cbu().await?;
        let issuer_id = db.create_company(&issuer_name).await?;

        // Create the share class the way the extended `share-class.create`
        // CRUD verb now maps its args (issuer-entity-id/instrument-kind/
        // votes-per-unit/economic-per-unit) -- proves those columns accept
        // real values via the only reachable creation path.
        let share_class_id: Uuid = sqlx::query_scalar!(
            r#"
            INSERT INTO "ob-poc".share_classes (
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

        let tx = db.pool.begin().await?;
        let mut scope = RollbackScope {
            id: ob_poc_types::TransactionScopeId::new(),
            tx,
            pool: db.pool.clone(),
        };
        let mut ctx = VerbExecutionContext::new(Principal::system());

        // capital.issue.initial -- first issuance for this class. Prior to
        // this pass this errored: its final statement was
        // `UPDATE share_classes SET issued_shares = ...`, a column that
        // doesn't exist. That statement is now deleted; this call proves
        // the fix by actually succeeding.
        let outcome = IssueInitial
            .execute(
                &serde_json::json!({
                    "share-class-id": share_class_id,
                    "units": "1000",
                    "price-per-unit": "10.00",
                }),
                &mut ctx,
                &mut scope,
            )
            .await?;
        assert!(matches!(outcome, VerbExecutionOutcome::Uuid(_)));

        // A second capital.issue.initial on the same class must be refused
        // (prior_exists guard) -- this is the real distinct job issue.initial
        // does that issue.new doesn't, the reason it survived as its own verb.
        let second_initial = IssueInitial
            .execute(
                &serde_json::json!({
                    "share-class-id": share_class_id,
                    "units": "1",
                }),
                &mut ctx,
                &mut scope,
            )
            .await;
        assert!(second_initial.is_err());

        // capital.issue.new -- subsequent issuance, running supply rollup.
        let outcome = IssueNew
            .execute(
                &serde_json::json!({
                    "share-class-id": share_class_id,
                    "units": "500",
                    "price-per-unit": "12.00",
                }),
                &mut ctx,
                &mut scope,
            )
            .await?;
        assert!(matches!(outcome, VerbExecutionOutcome::Uuid(_)));

        // Read-your-own-writes inside the still-open (uncommitted) scope:
        // 2 issuance_events rows (INITIAL_ISSUE + NEW_ISSUE, the failed
        // second-initial attempt never wrote anything), and
        // share_class_supply's latest row reflects both issuances summed.
        let event_count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".issuance_events WHERE share_class_id = $1"#,
        )
        .bind(share_class_id)
        .fetch_one(scope.executor())
        .await?;
        assert_eq!(event_count, 2);

        let issued: rust_decimal::Decimal = sqlx::query_scalar(
            r#"SELECT issued_units FROM "ob-poc".share_class_supply
               WHERE share_class_id = $1 ORDER BY as_of_date DESC, updated_at DESC LIMIT 1"#,
        )
        .bind(share_class_id)
        .fetch_one(scope.executor())
        .await?;
        assert_eq!(issued, rust_decimal::Decimal::from(1500));

        // Scope drops here without commit -- the two issuance_events/
        // share_class_supply writes above roll back. Only the share_classes/
        // entities/cbu setup rows (committed via db.pool directly) need
        // explicit cleanup.
        drop(scope);
        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // RECONCILE + SPLIT (EOP-PLAN share-register board design pass, 2026-08-19)
    //
    // Both were broken: Reconcile referenced share_classes.issued_shares/
    // .voting_rights_per_share (neither column exists); Split called
    // "ob-poc".uuid_to_lock_id() (never existed). Both fixed to use real
    // columns/mechanisms; these tests dispatch through the real SemOsVerbOp
    // layer to prove it.
    // =========================================================================

    #[tokio::test]
    async fn test_reconcile_against_real_holdings() -> Result<()> {
        use crate::ops::capital::Reconcile;
        use crate::ops::SemOsVerbOp;
        use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
        use sem_os_core::principal::Principal;
        use serde_json::Value;

        let db = TestDb::new().await?;
        let issuer_name = db.name("reconcile_issuer");
        let class_name = db.name("reconcile_class");
        let holder_a = db.name("reconcile_holder_a");
        let holder_b = db.name("reconcile_holder_b");

        let cbu_id = db.get_or_create_cbu().await?;
        let issuer_id = db.create_company(&issuer_name).await?;
        let holder_a_id = db.create_company(&holder_a).await?;
        let holder_b_id = db.create_company(&holder_b).await?;

        let share_class_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".share_classes (
                cbu_id, issuer_entity_id, name, instrument_kind, votes_per_unit
            ) VALUES ($1, $2, $3, 'ORDINARY_EQUITY', 1.0)
            RETURNING id
            "#,
        )
        .bind(cbu_id)
        .bind(issuer_id)
        .bind(&class_name)
        .fetch_one(&db.pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".share_class_supply (
                share_class_id, issued_units, outstanding_units, as_of_date
            ) VALUES ($1, 1000, 1000, CURRENT_DATE)
            "#,
        )
        .bind(share_class_id)
        .execute(&db.pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".holdings (share_class_id, investor_entity_id, units, status)
            VALUES ($1, $2, 600, 'active')
            "#,
        )
        .bind(share_class_id)
        .bind(holder_a_id)
        .execute(&db.pool)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".holdings (share_class_id, investor_entity_id, units, status)
            VALUES ($1, $2, 400, 'active')
            "#,
        )
        .bind(share_class_id)
        .bind(holder_b_id)
        .execute(&db.pool)
        .await?;

        let tx = db.pool.begin().await?;
        let mut scope = RollbackScope {
            id: ob_poc_types::TransactionScopeId::new(),
            tx,
            pool: db.pool.clone(),
        };
        let mut ctx = VerbExecutionContext::new(Principal::system());

        let outcome = Reconcile
            .execute(
                &serde_json::json!({ "entity-id": issuer_id }),
                &mut ctx,
                &mut scope,
            )
            .await?;
        let record = match outcome {
            VerbExecutionOutcome::Record(v) => v,
            other => panic!("expected Record, got {other:?}"),
        };

        let parse_dec = |v: &Value| -> rust_decimal::Decimal {
            v.as_str().unwrap().parse().unwrap()
        };

        assert_eq!(record["is_reconciled"], true);
        assert_eq!(parse_dec(&record["issued_shares"]), rust_decimal::Decimal::from(1000));
        assert_eq!(parse_dec(&record["allocated_shares"]), rust_decimal::Decimal::from(1000));
        let shareholders = record["shareholders"].as_array().expect("shareholders array");
        assert_eq!(shareholders.len(), 2);
        for sh in shareholders {
            let units = parse_dec(&sh["total_units"]);
            let pct: f64 = sh["ownership_pct"].as_str().unwrap().parse().unwrap();
            if units == rust_decimal::Decimal::from(600) {
                assert!((pct - 60.0).abs() < 0.01);
            } else if units == rust_decimal::Decimal::from(400) {
                assert!((pct - 40.0).abs() < 0.01);
            } else {
                panic!("unexpected holding units {units}");
            }
        }

        drop(scope);
        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_split_advisory_lock_and_supply_doubling() -> Result<()> {
        use crate::ops::capital::Split;
        use crate::ops::SemOsVerbOp;
        use dsl_runtime::{TransactionScope, VerbExecutionContext, VerbExecutionOutcome};
        use sem_os_core::principal::Principal;

        let db = TestDb::new().await?;
        let issuer_name = db.name("split_issuer");
        let class_name = db.name("split_class");

        let cbu_id = db.get_or_create_cbu().await?;
        let issuer_id = db.create_company(&issuer_name).await?;

        let share_class_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".share_classes (cbu_id, issuer_entity_id, name, instrument_kind)
            VALUES ($1, $2, $3, 'ORDINARY_EQUITY')
            RETURNING id
            "#,
        )
        .bind(cbu_id)
        .bind(issuer_id)
        .bind(&class_name)
        .fetch_one(&db.pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".share_class_supply (
                share_class_id, issued_units, outstanding_units, as_of_date
            ) VALUES ($1, 1000, 1000, CURRENT_DATE)
            "#,
        )
        .bind(share_class_id)
        .execute(&db.pool)
        .await?;

        let tx = db.pool.begin().await?;
        let mut scope = RollbackScope {
            id: ob_poc_types::TransactionScopeId::new(),
            tx,
            pool: db.pool.clone(),
        };
        let mut ctx = VerbExecutionContext::new(Principal::system());

        // 2-for-1 split -- proves the advisory lock call itself no longer
        // errors (the dead uuid_to_lock_id() SQL function is gone) and the
        // supply doubles correctly.
        let outcome = Split
            .execute(
                &serde_json::json!({
                    "share-class-id": share_class_id,
                    "ratio-from": 1,
                    "ratio-to": 2,
                }),
                &mut ctx,
                &mut scope,
            )
            .await?;
        let event_id = match outcome {
            VerbExecutionOutcome::Uuid(u) => u,
            other => panic!("expected Uuid, got {other:?}"),
        };

        let issued: rust_decimal::Decimal = sqlx::query_scalar(
            r#"SELECT issued_units FROM "ob-poc".share_class_supply
               WHERE share_class_id = $1 ORDER BY as_of_date DESC, updated_at DESC LIMIT 1"#,
        )
        .bind(share_class_id)
        .fetch_one(scope.executor())
        .await?;
        assert_eq!(issued, rust_decimal::Decimal::from(2000));

        // Idempotency: same share-class/ratio/date must return the same
        // event, not double the supply again.
        let outcome2 = Split
            .execute(
                &serde_json::json!({
                    "share-class-id": share_class_id,
                    "ratio-from": 1,
                    "ratio-to": 2,
                }),
                &mut ctx,
                &mut scope,
            )
            .await?;
        match outcome2 {
            VerbExecutionOutcome::Uuid(id) => assert_eq!(id, event_id),
            other => panic!("expected Uuid, got {other:?}"),
        }

        let issued_after_retry: rust_decimal::Decimal = sqlx::query_scalar(
            r#"SELECT issued_units FROM "ob-poc".share_class_supply
               WHERE share_class_id = $1 ORDER BY as_of_date DESC, updated_at DESC LIMIT 1"#,
        )
        .bind(share_class_id)
        .fetch_one(scope.executor())
        .await?;
        assert_eq!(issued_after_retry, rust_decimal::Decimal::from(2000));

        drop(scope);
        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // DILUTION REWRITE (EOP-PLAN share-register board design pass, Phase 3,
    // 2026-08-19) -- every create/list/forfeit op previously referenced
    // schema that never existed live and wrote status='OUTSTANDING', a
    // value the live CHECK constraint rejects outright. Exercises the real
    // grant -> partial exercise -> forfeit round trip through the actual
    // SemOsVerbOp layer, proving the unified ACTIVE/EXERCISED/FORFEITED
    // vocabulary, plus create-safe/create-convertible-note (units_granted=0,
    // proving the relaxed CHECK constraint), list, and get-summary.
    // =========================================================================

    #[tokio::test]
    async fn test_dilution_grant_exercise_forfeit_roundtrip() -> Result<()> {
        use crate::ops::dilution::{
            CreateConvertibleNote, CreateSafe, GetSummary, GrantOptions, IssueWarrant, List,
        };
        use crate::ops::SemOsVerbOp;
        use dsl_runtime::{TransactionScope, VerbExecutionContext, VerbExecutionOutcome};
        use sem_os_core::principal::Principal;

        let db = TestDb::new().await?;
        let issuer_name = db.name("dilution_issuer");
        let holder_name = db.name("dilution_holder");
        let class_name = db.name("dilution_class");

        let cbu_id = db.get_or_create_cbu().await?;
        let issuer_id = db.create_company(&issuer_name).await?;
        let holder_id = db.create_entity(&holder_name, "PROPER_PERSON_NATURAL").await?;

        let share_class_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".share_classes (cbu_id, issuer_entity_id, name, instrument_kind)
            VALUES ($1, $2, $3, 'ORDINARY_EQUITY')
            RETURNING id
            "#,
        )
        .bind(cbu_id)
        .bind(issuer_id)
        .bind(&class_name)
        .fetch_one(&db.pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".share_class_supply (
                share_class_id, issued_units, outstanding_units, as_of_date
            ) VALUES ($1, 10000, 10000, CURRENT_DATE)
            "#,
        )
        .bind(share_class_id)
        .execute(&db.pool)
        .await?;

        let tx = db.pool.begin().await?;
        let mut scope = RollbackScope {
            id: ob_poc_types::TransactionScopeId::new(),
            tx,
            pool: db.pool.clone(),
        };
        let mut ctx = VerbExecutionContext::new(Principal::system());

        // grant-options: 1000 STOCK_OPTION units. Previously errored on the
        // CHECK constraint (status='OUTSTANDING' is not a legal value).
        let option_outcome = GrantOptions
            .execute(
                &serde_json::json!({
                    "issuer-entity-id": issuer_id,
                    "converts-to-share-class-id": share_class_id,
                    "holder-entity-id": holder_id,
                    "units": "1000",
                    "exercise-price": "1.50",
                    "expiration-date": "2035-01-01",
                }),
                &mut ctx,
                &mut scope,
            )
            .await?;
        let option_id = match option_outcome {
            VerbExecutionOutcome::Uuid(u) => u,
            other => panic!("expected Uuid, got {other:?}"),
        };

        // issue-warrant: 500 WARRANT units, left ACTIVE (never exercised) --
        // exercises get-summary's ACTIVE-only aggregation below.
        let warrant_outcome = IssueWarrant
            .execute(
                &serde_json::json!({
                    "issuer-entity-id": issuer_id,
                    "converts-to-share-class-id": share_class_id,
                    "holder-entity-id": holder_id,
                    "units": "500",
                    "exercise-price": "2.00",
                    "expiration-date": "2035-01-01",
                }),
                &mut ctx,
                &mut scope,
            )
            .await?;
        assert!(matches!(warrant_outcome, VerbExecutionOutcome::Uuid(_)));

        // create-safe / create-convertible-note: units_granted=0 -- would
        // have violated the pre-fix CHECK (units_granted > 0).
        let safe_outcome = CreateSafe
            .execute(
                &serde_json::json!({
                    "issuer-entity-id": issuer_id,
                    "holder-entity-id": holder_id,
                    "principal-amount": "50000",
                    "valuation-cap": "5000000",
                    "discount-pct": "20",
                }),
                &mut ctx,
                &mut scope,
            )
            .await?;
        assert!(matches!(safe_outcome, VerbExecutionOutcome::Uuid(_)));

        let note_outcome = CreateConvertibleNote
            .execute(
                &serde_json::json!({
                    "issuer-entity-id": issuer_id,
                    "holder-entity-id": holder_id,
                    "principal-amount": "25000",
                    "expiration-date": "2028-01-01",
                }),
                &mut ctx,
                &mut scope,
            )
            .await?;
        assert!(matches!(note_outcome, VerbExecutionOutcome::Uuid(_)));

        // exercise 400 of the 1000 option units -- partial, stays ACTIVE.
        let exercise_outcome = crate::ops::dilution::Exercise
            .execute(
                &serde_json::json!({
                    "instrument-id": option_id,
                    "units": "400",
                }),
                &mut ctx,
                &mut scope,
            )
            .await?;
        assert!(matches!(exercise_outcome, VerbExecutionOutcome::Uuid(_)));

        let (units_exercised, status): (rust_decimal::Decimal, String) = sqlx::query_as(
            r#"SELECT units_exercised, status FROM "ob-poc".dilution_instruments WHERE instrument_id = $1"#,
        )
        .bind(option_id)
        .fetch_one(scope.executor())
        .await?;
        assert_eq!(units_exercised, rust_decimal::Decimal::from(400));
        assert_eq!(status, "ACTIVE");

        let holding_units: rust_decimal::Decimal = sqlx::query_scalar(
            r#"SELECT units FROM "ob-poc".holdings WHERE share_class_id = $1 AND investor_entity_id = $2"#,
        )
        .bind(share_class_id)
        .bind(holder_id)
        .fetch_one(scope.executor())
        .await?;
        assert_eq!(holding_units, rust_decimal::Decimal::from(400));

        // forfeit the remaining 600 -- full consumption (400 exercised + 600
        // forfeited == 1000 granted), terminal FORFEITED status.
        let forfeit_outcome = crate::ops::dilution::Forfeit
            .execute(
                &serde_json::json!({
                    "instrument-id": option_id,
                    "units": "600",
                    "reason": "employee departure",
                }),
                &mut ctx,
                &mut scope,
            )
            .await?;
        match forfeit_outcome {
            VerbExecutionOutcome::Affected(n) => assert_eq!(n, 1),
            other => panic!("expected Affected, got {other:?}"),
        }

        let (units_forfeited, status, notes): (rust_decimal::Decimal, String, Option<String>) =
            sqlx::query_as(
                r#"SELECT units_forfeited, status, notes FROM "ob-poc".dilution_instruments WHERE instrument_id = $1"#,
            )
            .bind(option_id)
            .fetch_one(scope.executor())
            .await?;
        assert_eq!(units_forfeited, rust_decimal::Decimal::from(600));
        assert_eq!(status, "FORFEITED");
        assert!(notes.unwrap_or_default().contains("employee departure"));

        // list ALL: 4 instruments (option forfeited, warrant + safe + note active).
        let list_outcome = List
            .execute(
                &serde_json::json!({ "issuer-entity-id": issuer_id, "status": "ALL" }),
                &mut ctx,
                &mut scope,
            )
            .await?;
        let records = match list_outcome {
            VerbExecutionOutcome::RecordSet(v) => v,
            other => panic!("expected RecordSet, got {other:?}"),
        };
        assert_eq!(records.len(), 4);

        // get-summary: ACTIVE-only aggregation -- the forfeited option is
        // excluded, the warrant (still ACTIVE) is included with a real
        // weighted_avg_strike.
        let summary_outcome = GetSummary
            .execute(
                &serde_json::json!({ "issuer-entity-id": issuer_id }),
                &mut ctx,
                &mut scope,
            )
            .await?;
        let summary = match summary_outcome {
            VerbExecutionOutcome::Record(v) => v,
            other => panic!("expected Record, got {other:?}"),
        };
        let by_type = summary["by_instrument_type"].as_array().expect("by_instrument_type array");
        let types: Vec<&str> = by_type
            .iter()
            .map(|e| e["instrument_type"].as_str().unwrap())
            .collect();
        assert!(types.contains(&"WARRANT"));
        assert!(!types.contains(&"STOCK_OPTION"));
        let warrant_entry = by_type
            .iter()
            .find(|e| e["instrument_type"] == "WARRANT")
            .unwrap();
        assert_eq!(warrant_entry["units_outstanding"], "500.000000");
        assert!(warrant_entry["weighted_avg_strike"].is_string());

        drop(scope);
        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // CLOSE-HOLDING (EOP-PLAN share-register board design pass, Phase 4) --
    // nothing previously closed a holding once its balance drained to zero;
    // it sat 'active' forever as a ghost row. New exit move, guarded to
    // require units = 0.
    // =========================================================================

    #[tokio::test]
    async fn test_close_holding_guards_nonzero_then_succeeds_at_zero() -> Result<()> {
        use crate::ops::capital::CloseHolding;
        use crate::ops::SemOsVerbOp;
        use dsl_runtime::{TransactionScope, VerbExecutionContext, VerbExecutionOutcome};
        use sem_os_core::principal::Principal;

        let db = TestDb::new().await?;
        let issuer_name = db.name("close_holding_issuer");
        let holder_name = db.name("close_holding_holder");
        let class_name = db.name("close_holding_class");

        let cbu_id = db.get_or_create_cbu().await?;
        let issuer_id = db.create_company(&issuer_name).await?;
        let holder_id = db.create_entity(&holder_name, "PROPER_PERSON_NATURAL").await?;

        let share_class_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".share_classes (cbu_id, issuer_entity_id, name, instrument_kind)
            VALUES ($1, $2, $3, 'ORDINARY_EQUITY')
            RETURNING id
            "#,
        )
        .bind(cbu_id)
        .bind(issuer_id)
        .bind(&class_name)
        .fetch_one(&db.pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".holdings (share_class_id, investor_entity_id, units, status)
            VALUES ($1, $2, 300, 'active')
            "#,
        )
        .bind(share_class_id)
        .bind(holder_id)
        .execute(&db.pool)
        .await?;

        let tx = db.pool.begin().await?;
        let mut scope = RollbackScope {
            id: ob_poc_types::TransactionScopeId::new(),
            tx,
            pool: db.pool.clone(),
        };
        let mut ctx = VerbExecutionContext::new(Principal::system());

        // guard: 300 units still outstanding -- must refuse.
        let refused = CloseHolding
            .execute(
                &serde_json::json!({
                    "share-class-id": share_class_id,
                    "shareholder-entity-id": holder_id,
                }),
                &mut ctx,
                &mut scope,
            )
            .await;
        assert!(refused.is_err());

        // drain the holding to zero, then close should succeed.
        sqlx::query(r#"UPDATE "ob-poc".holdings SET units = 0 WHERE share_class_id = $1 AND investor_entity_id = $2"#)
            .bind(share_class_id)
            .bind(holder_id)
            .execute(scope.executor())
            .await?;

        let outcome = CloseHolding
            .execute(
                &serde_json::json!({
                    "share-class-id": share_class_id,
                    "shareholder-entity-id": holder_id,
                }),
                &mut ctx,
                &mut scope,
            )
            .await?;
        match outcome {
            VerbExecutionOutcome::Affected(n) => assert_eq!(n, 1),
            other => panic!("expected Affected, got {other:?}"),
        }

        let status: String = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".holdings WHERE share_class_id = $1 AND investor_entity_id = $2"#,
        )
        .bind(share_class_id)
        .bind(holder_id)
        .fetch_one(scope.executor())
        .await?;
        assert_eq!(status, "closed");

        // idempotency guard: closing an already-closed holding must refuse.
        let already_closed = CloseHolding
            .execute(
                &serde_json::json!({
                    "share-class-id": share_class_id,
                    "shareholder-entity-id": holder_id,
                }),
                &mut ctx,
                &mut scope,
            )
            .await;
        assert!(already_closed.is_err());

        drop(scope);
        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // HARD_CLOSED/LIQUIDATED ISSUANCE GATE (EOP-PLAN share-register board
    // design pass, Phase 6) -- the DAG has always declared these as "no
    // subs AND no redemptions" / terminal, but nothing enforced it against
    // the issuance side until this pass.
    // =========================================================================

    #[tokio::test]
    async fn test_issuance_gate_refuses_hard_closed_and_liquidated() -> Result<()> {
        use crate::ops::capital::{Cancel, IssueNew};
        use crate::ops::SemOsVerbOp;
        use dsl_runtime::{TransactionScope, VerbExecutionContext};
        use sem_os_core::principal::Principal;

        let db = TestDb::new().await?;
        let issuer_name = db.name("gate_issuer");
        let class_name = db.name("gate_class");

        let cbu_id = db.get_or_create_cbu().await?;
        let issuer_id = db.create_company(&issuer_name).await?;

        let share_class_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".share_classes (cbu_id, issuer_entity_id, name, instrument_kind)
            VALUES ($1, $2, $3, 'ORDINARY_EQUITY')
            RETURNING id
            "#,
        )
        .bind(cbu_id)
        .bind(issuer_id)
        .bind(&class_name)
        .fetch_one(&db.pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".share_class_supply (
                share_class_id, issued_units, outstanding_units, as_of_date
            ) VALUES ($1, 1000, 1000, CURRENT_DATE)
            "#,
        )
        .bind(share_class_id)
        .execute(&db.pool)
        .await?;

        let tx = db.pool.begin().await?;
        let mut scope = RollbackScope {
            id: ob_poc_types::TransactionScopeId::new(),
            tx,
            pool: db.pool.clone(),
        };
        let mut ctx = VerbExecutionContext::new(Principal::system());

        // DRAFT (the real DEFAULT lifecycle_status) -- issuance succeeds;
        // the gate only refuses HARD_CLOSED/LIQUIDATED.
        let ok = IssueNew
            .execute(
                &serde_json::json!({
                    "share-class-id": share_class_id,
                    "units": "100",
                    "price-per-unit": "1.00",
                }),
                &mut ctx,
                &mut scope,
            )
            .await;
        assert!(ok.is_ok());

        sqlx::query(r#"UPDATE "ob-poc".share_classes SET lifecycle_status = 'HARD_CLOSED' WHERE id = $1"#)
            .bind(share_class_id)
            .execute(scope.executor())
            .await?;

        let refused_hard_closed = IssueNew
            .execute(
                &serde_json::json!({
                    "share-class-id": share_class_id,
                    "units": "100",
                    "price-per-unit": "1.00",
                }),
                &mut ctx,
                &mut scope,
            )
            .await;
        assert!(refused_hard_closed.is_err());

        sqlx::query(r#"UPDATE "ob-poc".share_classes SET lifecycle_status = 'LIQUIDATED' WHERE id = $1"#)
            .bind(share_class_id)
            .execute(scope.executor())
            .await?;

        let refused_liquidated = Cancel
            .execute(
                &serde_json::json!({
                    "share-class-id": share_class_id,
                    "units": "50",
                }),
                &mut ctx,
                &mut scope,
            )
            .await;
        assert!(refused_liquidated.is_err());

        // Confirm supply is untouched by the refused calls (no partial writes).
        let issued: rust_decimal::Decimal = sqlx::query_scalar(
            r#"SELECT issued_units FROM "ob-poc".share_class_supply
               WHERE share_class_id = $1 ORDER BY as_of_date DESC, updated_at DESC LIMIT 1"#,
        )
        .bind(share_class_id)
        .fetch_one(scope.executor())
        .await?;
        assert_eq!(issued, rust_decimal::Decimal::from(1100));

        drop(scope);
        db.cleanup().await?;
        Ok(())
    }
}
