//! Custody Domain Integration Tests
//!
//! Tests the three-layer custody model:
//! 1. Universe - What does the CBU trade?
//! 2. SSI Data - Standing Settlement Instructions
//! 3. Booking Rules - ALERT-style routing rules
//!
//! Run with: cargo test --features database --test custody_integration

use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::env;
use uuid::Uuid;

/// Test helper to set up database connection
async fn setup_pool() -> PgPool {
    dotenvy::dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database")
}

/// Test helper to create a test CBU with unique name
async fn create_test_cbu(pool: &PgPool, name: &str) -> Uuid {
    // Add UUID suffix to ensure uniqueness across parallel test runs
    let unique_name = format!("{} - {}", name, Uuid::now_v7());
    sqlx::query_scalar!(
        r#"
        INSERT INTO "ob-poc".cbus (name, jurisdiction, client_type)
        VALUES ($1, 'US', 'FUND')
        RETURNING cbu_id
        "#,
        unique_name
    )
    .fetch_one(pool)
    .await
    .expect("Failed to create test CBU")
}

/// Test helper to get instrument class ID
async fn get_instrument_class_id(pool: &PgPool, code: &str) -> Option<Uuid> {
    sqlx::query_scalar!(
        "SELECT class_id FROM custody.instrument_classes WHERE code = $1",
        code
    )
    .fetch_optional(pool)
    .await
    .expect("Failed to query instrument class")
}

/// Test helper to get market ID
async fn get_market_id(pool: &PgPool, mic: &str) -> Option<Uuid> {
    sqlx::query_scalar!("SELECT market_id FROM custody.markets WHERE mic = $1", mic)
        .fetch_optional(pool)
        .await
        .expect("Failed to query market")
}

/// Cleanup test data
async fn cleanup_test_cbu(pool: &PgPool, cbu_id: Uuid) {
    // Delete in order respecting foreign keys
    let _ = sqlx::query!(
        "DELETE FROM custody.ssi_booking_rules WHERE cbu_id = $1",
        cbu_id
    )
    .execute(pool)
    .await;

    let _ = sqlx::query!("DELETE FROM custody.cbu_ssi WHERE cbu_id = $1", cbu_id)
        .execute(pool)
        .await;

    let _ = sqlx::query!(
        "DELETE FROM custody.cbu_instrument_universe WHERE cbu_id = $1",
        cbu_id
    )
    .execute(pool)
    .await;

    let _ = sqlx::query!(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#, cbu_id)
        .execute(pool)
        .await;
}

// ============================================================================
// Layer 1: Universe Tests
// ============================================================================

#[tokio::test]
async fn test_add_universe_entry() {
    let pool = setup_pool().await;
    let cbu_id = create_test_cbu(&pool, "Test Fund - Universe").await;

    // Get instrument class ID
    let class_id = get_instrument_class_id(&pool, "EQUITY")
        .await
        .expect("EQUITY instrument class should exist");

    // Get market ID
    let market_id = get_market_id(&pool, "XNYS")
        .await
        .expect("XNYS market should exist");

    // Add universe entry
    let universe_id = sqlx::query_scalar!(
        r#"
        INSERT INTO custody.cbu_instrument_universe
            (cbu_id, instrument_class_id, market_id, currencies, settlement_types)
        VALUES ($1, $2, $3, ARRAY['USD'], ARRAY['DVP'])
        RETURNING universe_id
        "#,
        cbu_id,
        class_id,
        market_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to add universe entry");

    assert!(universe_id != Uuid::nil());

    // Verify entry exists
    let count: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM custody.cbu_instrument_universe WHERE cbu_id = $1",
        cbu_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to count universe entries")
    .unwrap_or(0);

    assert_eq!(count, 1);

    cleanup_test_cbu(&pool, cbu_id).await;
}

#[tokio::test]
async fn test_universe_with_multiple_currencies() {
    let pool = setup_pool().await;
    let cbu_id = create_test_cbu(&pool, "Test Fund - Multi Currency").await;

    let class_id = get_instrument_class_id(&pool, "EQUITY")
        .await
        .expect("EQUITY class should exist");

    let market_id = get_market_id(&pool, "XLON")
        .await
        .expect("XLON market should exist");

    // Add universe with GBP and USD (cross-currency trading)
    let universe_id = sqlx::query_scalar!(
        r#"
        INSERT INTO custody.cbu_instrument_universe
            (cbu_id, instrument_class_id, market_id, currencies, settlement_types)
        VALUES ($1, $2, $3, ARRAY['GBP', 'USD'], ARRAY['DVP', 'FOP'])
        RETURNING universe_id
        "#,
        cbu_id,
        class_id,
        market_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to add multi-currency universe");

    // Verify currencies array
    let row = sqlx::query!(
        "SELECT currencies FROM custody.cbu_instrument_universe WHERE universe_id = $1",
        universe_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch currencies");

    let currencies = row.currencies;

    assert_eq!(currencies.len(), 2);
    assert!(currencies.contains(&"GBP".to_string()));
    assert!(currencies.contains(&"USD".to_string()));

    cleanup_test_cbu(&pool, cbu_id).await;
}

// ============================================================================
// Layer 2: SSI Tests
// ============================================================================

#[tokio::test]
async fn test_create_ssi() {
    let pool = setup_pool().await;
    let cbu_id = create_test_cbu(&pool, "Test Fund - SSI").await;

    // Create SSI
    let ssi_id = sqlx::query_scalar!(
        r#"
        INSERT INTO custody.cbu_ssi (
            cbu_id, ssi_name, ssi_type,
            safekeeping_account, safekeeping_bic,
            cash_account, cash_account_bic, cash_currency,
            pset_bic, status, effective_date
        )
        VALUES ($1, 'US Equity SSI', 'SECURITIES',
                'SAFE-001', 'CITIUS33',
                'CASH-001', 'CITIUS33', 'USD',
                'DTCYUS33', 'ACTIVE', CURRENT_DATE)
        RETURNING ssi_id
        "#,
        cbu_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create SSI");

    assert!(ssi_id != Uuid::nil());

    // Verify SSI exists and is active
    let status: String = sqlx::query_scalar!(
        "SELECT status FROM custody.cbu_ssi WHERE ssi_id = $1",
        ssi_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch SSI status")
    .unwrap_or_default();

    assert_eq!(status, "ACTIVE");

    cleanup_test_cbu(&pool, cbu_id).await;
}

#[tokio::test]
async fn test_ssi_status_transitions() {
    let pool = setup_pool().await;
    let cbu_id = create_test_cbu(&pool, "Test Fund - SSI Status").await;

    // Create SSI in PENDING state
    let ssi_id = sqlx::query_scalar!(
        r#"
        INSERT INTO custody.cbu_ssi (
            cbu_id, ssi_name, ssi_type,
            safekeeping_account, safekeeping_bic,
            pset_bic, status, effective_date
        )
        VALUES ($1, 'Pending SSI', 'SECURITIES',
                'SAFE-002', 'BABOROCP',
                'DTCYUS33', 'PENDING', CURRENT_DATE)
        RETURNING ssi_id
        "#,
        cbu_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create pending SSI");

    // Activate SSI
    sqlx::query!(
        "UPDATE custody.cbu_ssi SET status = 'ACTIVE' WHERE ssi_id = $1",
        ssi_id
    )
    .execute(&pool)
    .await
    .expect("Failed to activate SSI");

    // Verify active
    let status: String = sqlx::query_scalar!(
        "SELECT status FROM custody.cbu_ssi WHERE ssi_id = $1",
        ssi_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch status")
    .unwrap_or_default();

    assert_eq!(status, "ACTIVE");

    // Suspend SSI
    sqlx::query!(
        "UPDATE custody.cbu_ssi SET status = 'SUSPENDED' WHERE ssi_id = $1",
        ssi_id
    )
    .execute(&pool)
    .await
    .expect("Failed to suspend SSI");

    let status: String = sqlx::query_scalar!(
        "SELECT status FROM custody.cbu_ssi WHERE ssi_id = $1",
        ssi_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch status")
    .unwrap_or_default();

    assert_eq!(status, "SUSPENDED");

    cleanup_test_cbu(&pool, cbu_id).await;
}

// ============================================================================
// Layer 3: Booking Rules Tests
// ============================================================================

#[tokio::test]
async fn test_create_booking_rule() {
    let pool = setup_pool().await;
    let cbu_id = create_test_cbu(&pool, "Test Fund - Booking Rule").await;

    // Create SSI first
    let ssi_id = sqlx::query_scalar!(
        r#"
        INSERT INTO custody.cbu_ssi (
            cbu_id, ssi_name, ssi_type,
            safekeeping_account, safekeeping_bic,
            pset_bic, status, effective_date
        )
        VALUES ($1, 'Rule Test SSI', 'SECURITIES',
                'SAFE-003', 'CITIUS33',
                'DTCYUS33', 'ACTIVE', CURRENT_DATE)
        RETURNING ssi_id
        "#,
        cbu_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create SSI");

    let class_id = get_instrument_class_id(&pool, "EQUITY")
        .await
        .expect("EQUITY class should exist");

    let market_id = get_market_id(&pool, "XNYS")
        .await
        .expect("XNYS market should exist");

    // Create specific booking rule
    let rule_id = sqlx::query_scalar!(
        r#"
        INSERT INTO custody.ssi_booking_rules (
            cbu_id, ssi_id, rule_name, priority,
            instrument_class_id, market_id, currency, settlement_type
        )
        VALUES ($1, $2, 'US Equity DVP', 10,
                $3, $4, 'USD', 'DVP')
        RETURNING rule_id
        "#,
        cbu_id,
        ssi_id,
        class_id,
        market_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create booking rule");

    assert!(rule_id != Uuid::nil());

    // Verify specificity score was computed (should be 4 - all fields specified)
    let specificity: i32 = sqlx::query_scalar!(
        "SELECT specificity_score FROM custody.ssi_booking_rules WHERE rule_id = $1",
        rule_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch specificity")
    .unwrap_or(0);

    // Specificity is a bitmask: instrument_class=16, market=4, currency=2, settlement_type=1
    // So 16 + 4 + 2 + 1 = 23
    assert_eq!(
        specificity, 23,
        "Specificity should be 23 (instrument_class=16 + market=4 + currency=2 + settlement_type=1)"
    );

    cleanup_test_cbu(&pool, cbu_id).await;
}

#[tokio::test]
async fn test_booking_rule_priority_ordering() {
    let pool = setup_pool().await;
    let cbu_id = create_test_cbu(&pool, "Test Fund - Rule Priority").await;

    // Create SSI
    let ssi_id = sqlx::query_scalar!(
        r#"
        INSERT INTO custody.cbu_ssi (
            cbu_id, ssi_name, ssi_type,
            safekeeping_account, safekeeping_bic,
            pset_bic, status, effective_date
        )
        VALUES ($1, 'Priority Test SSI', 'SECURITIES',
                'SAFE-004', 'CITIUS33',
                'DTCYUS33', 'ACTIVE', CURRENT_DATE)
        RETURNING ssi_id
        "#,
        cbu_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create SSI");

    let class_id = get_instrument_class_id(&pool, "EQUITY")
        .await
        .expect("EQUITY class should exist");

    // Create high priority specific rule (priority 10)
    let _rule1 = sqlx::query_scalar!(
        r#"
        INSERT INTO custody.ssi_booking_rules (
            cbu_id, ssi_id, rule_name, priority,
            instrument_class_id, currency
        )
        VALUES ($1, $2, 'Specific USD Equity', 10, $3, 'USD')
        RETURNING rule_id
        "#,
        cbu_id,
        ssi_id,
        class_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create rule 1");

    // Create lower priority fallback rule (priority 50)
    let _rule2 = sqlx::query_scalar!(
        r#"
        INSERT INTO custody.ssi_booking_rules (
            cbu_id, ssi_id, rule_name, priority,
            currency
        )
        VALUES ($1, $2, 'USD Fallback', 50, 'USD')
        RETURNING rule_id
        "#,
        cbu_id,
        ssi_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create rule 2");

    // Query rules ordered by priority
    let rules = sqlx::query!(
        r#"
        SELECT rule_name, priority, specificity_score
        FROM custody.ssi_booking_rules
        WHERE cbu_id = $1
        ORDER BY priority ASC, specificity_score DESC
        "#,
        cbu_id
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to fetch rules");

    assert_eq!(rules.len(), 2);
    assert_eq!(rules[0].rule_name, "Specific USD Equity");
    assert_eq!(rules[0].priority, 10);
    assert_eq!(rules[1].rule_name, "USD Fallback");
    assert_eq!(rules[1].priority, 50);

    // The specific rule should have higher specificity
    assert!(rules[0].specificity_score.unwrap_or(0) > rules[1].specificity_score.unwrap_or(0));

    cleanup_test_cbu(&pool, cbu_id).await;
}

#[tokio::test]
async fn test_wildcard_booking_rule() {
    let pool = setup_pool().await;
    let cbu_id = create_test_cbu(&pool, "Test Fund - Wildcard Rule").await;

    // Create SSI
    let ssi_id = sqlx::query_scalar!(
        r#"
        INSERT INTO custody.cbu_ssi (
            cbu_id, ssi_name, ssi_type,
            safekeeping_account, safekeeping_bic,
            pset_bic, status, effective_date
        )
        VALUES ($1, 'Wildcard Test SSI', 'SECURITIES',
                'SAFE-005', 'CITIUS33',
                'DTCYUS33', 'ACTIVE', CURRENT_DATE)
        RETURNING ssi_id
        "#,
        cbu_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create SSI");

    // Create wildcard rule (all NULLs = catch-all)
    let rule_id = sqlx::query_scalar!(
        r#"
        INSERT INTO custody.ssi_booking_rules (
            cbu_id, ssi_id, rule_name, priority
        )
        VALUES ($1, $2, 'Catch-All Rule', 100)
        RETURNING rule_id
        "#,
        cbu_id,
        ssi_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create wildcard rule");

    // Specificity should be 0 (no criteria specified)
    let specificity: i32 = sqlx::query_scalar!(
        "SELECT specificity_score FROM custody.ssi_booking_rules WHERE rule_id = $1",
        rule_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to fetch specificity")
    .unwrap_or(-1);

    assert_eq!(specificity, 0, "Wildcard rule should have 0 specificity");

    cleanup_test_cbu(&pool, cbu_id).await;
}

// ============================================================================
// SSI Lookup Tests (ALERT-style matching)
// ============================================================================

#[tokio::test]
async fn test_ssi_lookup_exact_match() {
    let pool = setup_pool().await;
    let cbu_id = create_test_cbu(&pool, "Test Fund - SSI Lookup").await;

    // Create SSI
    let ssi_id = sqlx::query_scalar!(
        r#"
        INSERT INTO custody.cbu_ssi (
            cbu_id, ssi_name, ssi_type,
            safekeeping_account, safekeeping_bic,
            pset_bic, status, effective_date
        )
        VALUES ($1, 'Lookup Test SSI', 'SECURITIES',
                'SAFE-006', 'CITIUS33',
                'DTCYUS33', 'ACTIVE', CURRENT_DATE)
        RETURNING ssi_id
        "#,
        cbu_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create SSI");

    let class_id = get_instrument_class_id(&pool, "EQUITY")
        .await
        .expect("EQUITY class should exist");

    let market_id = get_market_id(&pool, "XNYS")
        .await
        .expect("XNYS market should exist");

    // Create specific rule
    sqlx::query!(
        r#"
        INSERT INTO custody.ssi_booking_rules (
            cbu_id, ssi_id, rule_name, priority,
            instrument_class_id, market_id, currency, settlement_type
        )
        VALUES ($1, $2, 'Exact Match Rule', 10, $3, $4, 'USD', 'DVP')
        "#,
        cbu_id,
        ssi_id,
        class_id,
        market_id
    )
    .execute(&pool)
    .await
    .expect("Failed to create rule");

    // Use the find_ssi_for_trade function if it exists, otherwise manual query
    let result = sqlx::query!(
        r#"
        SELECT r.ssi_id, r.rule_name, s.ssi_name
        FROM custody.ssi_booking_rules r
        JOIN custody.cbu_ssi s ON s.ssi_id = r.ssi_id
        WHERE r.cbu_id = $1
          AND r.is_active = true
          AND s.status = 'ACTIVE'
          AND (r.instrument_class_id IS NULL OR r.instrument_class_id = $2)
          AND (r.market_id IS NULL OR r.market_id = $3)
          AND (r.currency IS NULL OR r.currency = $4)
          AND (r.settlement_type IS NULL OR r.settlement_type = $5)
        ORDER BY r.priority ASC, r.specificity_score DESC
        LIMIT 1
        "#,
        cbu_id,
        class_id,
        market_id,
        "USD",
        "DVP"
    )
    .fetch_optional(&pool)
    .await
    .expect("Failed to lookup SSI");

    assert!(result.is_some(), "Should find matching SSI");
    let row = result.unwrap();
    assert_eq!(row.ssi_name, "Lookup Test SSI");
    assert_eq!(row.rule_name, "Exact Match Rule");

    cleanup_test_cbu(&pool, cbu_id).await;
}

#[tokio::test]
async fn test_ssi_lookup_fallback_to_wildcard() {
    let pool = setup_pool().await;
    let cbu_id = create_test_cbu(&pool, "Test Fund - Fallback Lookup").await;

    // Create SSI
    let ssi_id = sqlx::query_scalar!(
        r#"
        INSERT INTO custody.cbu_ssi (
            cbu_id, ssi_name, ssi_type,
            safekeeping_account, safekeeping_bic,
            pset_bic, status, effective_date
        )
        VALUES ($1, 'Fallback SSI', 'SECURITIES',
                'SAFE-007', 'CITIUS33',
                'DTCYUS33', 'ACTIVE', CURRENT_DATE)
        RETURNING ssi_id
        "#,
        cbu_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create SSI");

    let class_id = get_instrument_class_id(&pool, "EQUITY")
        .await
        .expect("EQUITY class should exist");

    let market_id = get_market_id(&pool, "XNYS")
        .await
        .expect("XNYS market should exist");

    // Create specific rule for XLON (wrong market)
    let xlon_market_id = get_market_id(&pool, "XLON").await;
    if let Some(xlon_id) = xlon_market_id {
        sqlx::query!(
            r#"
            INSERT INTO custody.ssi_booking_rules (
                cbu_id, ssi_id, rule_name, priority,
                instrument_class_id, market_id, currency
            )
            VALUES ($1, $2, 'XLON Specific', 10, $3, $4, 'GBP')
            "#,
            cbu_id,
            ssi_id,
            class_id,
            xlon_id
        )
        .execute(&pool)
        .await
        .expect("Failed to create XLON rule");
    }

    // Create wildcard fallback rule
    sqlx::query!(
        r#"
        INSERT INTO custody.ssi_booking_rules (
            cbu_id, ssi_id, rule_name, priority
        )
        VALUES ($1, $2, 'Catch-All Fallback', 100)
        "#,
        cbu_id,
        ssi_id
    )
    .execute(&pool)
    .await
    .expect("Failed to create fallback rule");

    // Look up for XNYS/USD - should fall back to wildcard
    let result = sqlx::query!(
        r#"
        SELECT r.rule_name, r.priority
        FROM custody.ssi_booking_rules r
        JOIN custody.cbu_ssi s ON s.ssi_id = r.ssi_id
        WHERE r.cbu_id = $1
          AND r.is_active = true
          AND s.status = 'ACTIVE'
          AND (r.instrument_class_id IS NULL OR r.instrument_class_id = $2)
          AND (r.market_id IS NULL OR r.market_id = $3)
          AND (r.currency IS NULL OR r.currency = $4)
        ORDER BY r.priority ASC, r.specificity_score DESC
        LIMIT 1
        "#,
        cbu_id,
        class_id,
        market_id,
        "USD"
    )
    .fetch_optional(&pool)
    .await
    .expect("Failed to lookup SSI");

    assert!(result.is_some(), "Should find fallback SSI");
    let row = result.unwrap();
    assert_eq!(row.rule_name, "Catch-All Fallback");
    assert_eq!(row.priority, 100);

    cleanup_test_cbu(&pool, cbu_id).await;
}

// ============================================================================
// Coverage Validation Tests
// ============================================================================

#[tokio::test]
async fn test_coverage_validation_complete() {
    let pool = setup_pool().await;
    let cbu_id = create_test_cbu(&pool, "Test Fund - Complete Coverage").await;

    let class_id = get_instrument_class_id(&pool, "EQUITY")
        .await
        .expect("EQUITY class should exist");

    let market_id = get_market_id(&pool, "XNYS")
        .await
        .expect("XNYS market should exist");

    // Add universe entry
    sqlx::query!(
        r#"
        INSERT INTO custody.cbu_instrument_universe
            (cbu_id, instrument_class_id, market_id, currencies, settlement_types)
        VALUES ($1, $2, $3, ARRAY['USD'], ARRAY['DVP'])
        "#,
        cbu_id,
        class_id,
        market_id
    )
    .execute(&pool)
    .await
    .expect("Failed to add universe");

    // Create matching SSI and rule
    let ssi_id = sqlx::query_scalar!(
        r#"
        INSERT INTO custody.cbu_ssi (
            cbu_id, ssi_name, ssi_type,
            safekeeping_account, safekeeping_bic,
            pset_bic, status, effective_date
        )
        VALUES ($1, 'Coverage SSI', 'SECURITIES',
                'SAFE-008', 'CITIUS33',
                'DTCYUS33', 'ACTIVE', CURRENT_DATE)
        RETURNING ssi_id
        "#,
        cbu_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create SSI");

    sqlx::query!(
        r#"
        INSERT INTO custody.ssi_booking_rules (
            cbu_id, ssi_id, rule_name, priority,
            instrument_class_id, market_id
        )
        VALUES ($1, $2, 'Covering Rule', 10, $3, $4)
        "#,
        cbu_id,
        ssi_id,
        class_id,
        market_id
    )
    .execute(&pool)
    .await
    .expect("Failed to create rule");

    // Check for gaps (should be none)
    let gaps = sqlx::query!(
        r#"
        SELECT u.universe_id
        FROM custody.cbu_instrument_universe u
        WHERE u.cbu_id = $1
          AND u.is_active = true
          AND NOT EXISTS (
              SELECT 1 FROM custody.ssi_booking_rules r
              WHERE r.cbu_id = u.cbu_id
                AND r.is_active = true
                AND (r.instrument_class_id IS NULL OR r.instrument_class_id = u.instrument_class_id)
                AND (r.market_id IS NULL OR r.market_id = u.market_id)
          )
        "#,
        cbu_id
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to check coverage");

    assert!(gaps.is_empty(), "Coverage should be complete (no gaps)");

    cleanup_test_cbu(&pool, cbu_id).await;
}

#[tokio::test]
async fn test_coverage_validation_with_gap() {
    let pool = setup_pool().await;
    let cbu_id = create_test_cbu(&pool, "Test Fund - Coverage Gap").await;

    let equity_class_id = get_instrument_class_id(&pool, "EQUITY")
        .await
        .expect("EQUITY class should exist");

    let bond_class_id = get_instrument_class_id(&pool, "GOVT_BOND").await.or({
        // GOVT_BOND might not exist, skip test
        None
    });

    let market_id = get_market_id(&pool, "XNYS")
        .await
        .expect("XNYS market should exist");

    // Add universe entries for both EQUITY and GOVT_BOND
    sqlx::query!(
        r#"
        INSERT INTO custody.cbu_instrument_universe
            (cbu_id, instrument_class_id, market_id, currencies, settlement_types)
        VALUES ($1, $2, $3, ARRAY['USD'], ARRAY['DVP'])
        "#,
        cbu_id,
        equity_class_id,
        market_id
    )
    .execute(&pool)
    .await
    .expect("Failed to add EQUITY universe");

    if let Some(bond_id) = bond_class_id {
        sqlx::query!(
            r#"
            INSERT INTO custody.cbu_instrument_universe
                (cbu_id, instrument_class_id, market_id, currencies, settlement_types)
            VALUES ($1, $2, $3, ARRAY['USD'], ARRAY['DVP'])
            "#,
            cbu_id,
            bond_id,
            market_id
        )
        .execute(&pool)
        .await
        .expect("Failed to add GOVT_BOND universe");
    }

    // Create SSI and rule ONLY for EQUITY (gap for GOVT_BOND)
    let ssi_id = sqlx::query_scalar!(
        r#"
        INSERT INTO custody.cbu_ssi (
            cbu_id, ssi_name, ssi_type,
            safekeeping_account, safekeeping_bic,
            pset_bic, status, effective_date
        )
        VALUES ($1, 'Equity Only SSI', 'SECURITIES',
                'SAFE-009', 'CITIUS33',
                'DTCYUS33', 'ACTIVE', CURRENT_DATE)
        RETURNING ssi_id
        "#,
        cbu_id
    )
    .fetch_one(&pool)
    .await
    .expect("Failed to create SSI");

    sqlx::query!(
        r#"
        INSERT INTO custody.ssi_booking_rules (
            cbu_id, ssi_id, rule_name, priority,
            instrument_class_id, market_id
        )
        VALUES ($1, $2, 'Equity Only Rule', 10, $3, $4)
        "#,
        cbu_id,
        ssi_id,
        equity_class_id,
        market_id
    )
    .execute(&pool)
    .await
    .expect("Failed to create rule");

    // Check for gaps
    let gaps = sqlx::query!(
        r#"
        SELECT u.universe_id
        FROM custody.cbu_instrument_universe u
        WHERE u.cbu_id = $1
          AND u.is_active = true
          AND NOT EXISTS (
              SELECT 1 FROM custody.ssi_booking_rules r
              WHERE r.cbu_id = u.cbu_id
                AND r.is_active = true
                AND (r.instrument_class_id IS NULL OR r.instrument_class_id = u.instrument_class_id)
                AND (r.market_id IS NULL OR r.market_id = u.market_id)
          )
        "#,
        cbu_id
    )
    .fetch_all(&pool)
    .await
    .expect("Failed to check coverage");

    // If GOVT_BOND exists in universe, we should have a gap
    if bond_class_id.is_some() {
        assert_eq!(gaps.len(), 1, "Should have 1 gap (GOVT_BOND not covered)");
    }

    cleanup_test_cbu(&pool, cbu_id).await;
}
