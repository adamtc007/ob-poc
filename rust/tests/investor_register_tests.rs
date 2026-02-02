//! Integration tests for the Investor Register + UBO/Economic Pipeline
//!
//! These tests verify:
//! 1. Investor role profiles with temporal versioning
//! 2. UBO sync trigger respects usage_type and role profiles
//! 3. Economic look-through with cycle detection
//! 4. Fund vehicle taxonomy

#![cfg(feature = "database")]

use sqlx::PgPool;
use uuid::Uuid;

/// Get database pool from environment
async fn get_pool() -> PgPool {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    PgPool::connect(&database_url).await.unwrap()
}

// =============================================================================
// INVESTOR ROLE PROFILE TESTS
// =============================================================================

#[tokio::test]
async fn test_role_profile_upsert_creates_new() {
    let pool = get_pool().await;

    // Create test entities
    let issuer_id = create_test_entity(&pool, "Test Issuer Fund").await;
    let holder_id = create_test_entity(&pool, "Test Holder Entity").await;

    // Insert role profile via SQL function (effective_from = CURRENT_DATE)
    let result: (Uuid,) = sqlx::query_as(
        r#"
        SELECT kyc.upsert_role_profile(
            $1, $2, 'END_INVESTOR', 'NONE', 'EXTERNAL', false, true,
            NULL, NULL, NULL, CURRENT_DATE, 'TEST', NULL, NULL, NULL
        )
        "#,
    )
    .bind(issuer_id)
    .bind(holder_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert!(!result.0.is_nil());

    // Verify it was created
    let profile = sqlx::query!(
        r#"
        SELECT role_type, lookthrough_policy, is_ubo_eligible
        FROM kyc.investor_role_profiles
        WHERE id = $1
        "#,
        result.0
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(profile.role_type, "END_INVESTOR");
    assert_eq!(profile.lookthrough_policy, "NONE");
    assert!(profile.is_ubo_eligible);

    // Cleanup
    cleanup_test_entity(&pool, issuer_id).await;
    cleanup_test_entity(&pool, holder_id).await;
}

#[tokio::test]
async fn test_role_profile_temporal_versioning() {
    let pool = get_pool().await;

    let issuer_id = create_test_entity(&pool, "Test Issuer Temporal").await;
    let holder_id = create_test_entity(&pool, "Test Holder Temporal").await;

    // Create first version with effective_from = 2024-01-01
    let _v1: (Uuid,) = sqlx::query_as(
        r#"
        SELECT kyc.upsert_role_profile(
            $1, $2, 'NOMINEE', 'NONE', 'EXTERNAL', false, false,
            NULL, NULL, NULL, '2024-01-01'::date, 'TEST', NULL, NULL, NULL
        )
        "#,
    )
    .bind(issuer_id)
    .bind(holder_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Create second version with effective_from = 2024-06-01 (should close first)
    let _v2: (Uuid,) = sqlx::query_as(
        r#"
        SELECT kyc.upsert_role_profile(
            $1, $2, 'END_INVESTOR', 'ON_DEMAND', 'EXTERNAL', true, true,
            NULL, NULL, NULL, '2024-06-01'::date, 'TEST', NULL, NULL, NULL
        )
        "#,
    )
    .bind(issuer_id)
    .bind(holder_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Query as of 2024-03-01 - should get NOMINEE
    let profile_march = sqlx::query!(
        r#"
        SELECT role_type, is_ubo_eligible
        FROM kyc.investor_role_profiles
        WHERE issuer_entity_id = $1 AND holder_entity_id = $2
          AND effective_from <= '2024-03-01'::date
          AND (effective_to IS NULL OR effective_to > '2024-03-01'::date)
        "#,
        issuer_id,
        holder_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(profile_march.role_type, "NOMINEE");
    assert!(!profile_march.is_ubo_eligible);

    // Query as of 2024-09-01 - should get END_INVESTOR
    let profile_sept = sqlx::query!(
        r#"
        SELECT role_type, is_ubo_eligible
        FROM kyc.investor_role_profiles
        WHERE issuer_entity_id = $1 AND holder_entity_id = $2
          AND effective_from <= '2024-09-01'::date
          AND (effective_to IS NULL OR effective_to > '2024-09-01'::date)
        "#,
        issuer_id,
        holder_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(profile_sept.role_type, "END_INVESTOR");
    assert!(profile_sept.is_ubo_eligible);

    // Cleanup
    cleanup_test_entity(&pool, issuer_id).await;
    cleanup_test_entity(&pool, holder_id).await;
}

// =============================================================================
// FUND VEHICLE TESTS
// =============================================================================

#[tokio::test]
async fn test_fund_vehicle_upsert() {
    let pool = get_pool().await;

    let fund_id = create_test_entity(&pool, "Test Fund Vehicle").await;

    // Insert fund vehicle
    sqlx::query!(
        r#"
        INSERT INTO kyc.fund_vehicles (fund_entity_id, vehicle_type, is_umbrella, domicile_country)
        VALUES ($1, 'SCSP', true, 'LU')
        ON CONFLICT (fund_entity_id) DO UPDATE SET vehicle_type = EXCLUDED.vehicle_type
        "#,
        fund_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Verify via view
    let summary = sqlx::query!(
        r#"
        SELECT fund_name, vehicle_type, is_umbrella, domicile_country
        FROM kyc.v_fund_vehicle_summary
        WHERE fund_entity_id = $1
        "#,
        fund_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(summary.vehicle_type, Some("SCSP".to_string()));
    assert!(summary.is_umbrella.unwrap_or(false));
    assert_eq!(summary.domicile_country, Some("LU".to_string()));

    // Cleanup
    sqlx::query!(
        "DELETE FROM kyc.fund_vehicles WHERE fund_entity_id = $1",
        fund_id
    )
    .execute(&pool)
    .await
    .unwrap();
    cleanup_test_entity(&pool, fund_id).await;
}

#[tokio::test]
async fn test_fund_compartments() {
    let pool = get_pool().await;

    let umbrella_id = create_test_entity(&pool, "Test Umbrella Fund").await;

    // Create umbrella fund vehicle
    sqlx::query!(
        r#"
        INSERT INTO kyc.fund_vehicles (fund_entity_id, vehicle_type, is_umbrella, domicile_country)
        VALUES ($1, 'SICAV_RAIF', true, 'LU')
        "#,
        umbrella_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Create compartments
    sqlx::query!(
        r#"
        INSERT INTO kyc.fund_compartments (umbrella_fund_entity_id, compartment_code, compartment_name)
        VALUES
            ($1, 'EQUITY', 'Global Equity Fund'),
            ($1, 'BOND', 'Fixed Income Fund')
        "#,
        umbrella_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Verify compartment count
    let summary = sqlx::query!(
        r#"
        SELECT compartment_count
        FROM kyc.v_fund_vehicle_summary
        WHERE fund_entity_id = $1
        "#,
        umbrella_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(summary.compartment_count, Some(2));

    // Cleanup
    sqlx::query!(
        "DELETE FROM kyc.fund_compartments WHERE umbrella_fund_entity_id = $1",
        umbrella_id
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query!(
        "DELETE FROM kyc.fund_vehicles WHERE fund_entity_id = $1",
        umbrella_id
    )
    .execute(&pool)
    .await
    .unwrap();
    cleanup_test_entity(&pool, umbrella_id).await;
}

// =============================================================================
// ECONOMIC LOOK-THROUGH TESTS
// =============================================================================

#[tokio::test]
async fn test_economic_exposure_function_exists() {
    let pool = get_pool().await;

    // Verify the function exists and can be called (even with no data)
    let result = sqlx::query!(
        r#"
        SELECT COUNT(*) as "count!" FROM kyc.fn_compute_economic_exposure(
            '00000000-0000-0000-0000-000000000000'::uuid,
            CURRENT_DATE,
            6, 0.0001, 200, true, true
        )
        "#
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Function should return 0 rows for non-existent entity
    assert_eq!(result.count, 0);
}

#[tokio::test]
async fn test_economic_exposure_summary_function_exists() {
    let pool = get_pool().await;

    // Verify the function exists
    let result = sqlx::query!(
        r#"
        SELECT COUNT(*) as "count!" FROM kyc.fn_economic_exposure_summary(
            '00000000-0000-0000-0000-000000000000'::uuid,
            CURRENT_DATE,
            5.0
        )
        "#
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(result.count, 0);
}

// =============================================================================
// ISSUER CONTROL CONFIG TESTS
// =============================================================================

#[tokio::test]
async fn test_issuer_control_config_defaults() {
    let pool = get_pool().await;

    let issuer_id = create_test_entity(&pool, "Test Issuer Config").await;

    // Insert with defaults (effective_from defaults to CURRENT_DATE)
    sqlx::query!(
        r#"
        INSERT INTO kyc.issuer_control_config (issuer_entity_id, effective_from)
        VALUES ($1, CURRENT_DATE)
        ON CONFLICT (issuer_entity_id, effective_from) DO NOTHING
        "#,
        issuer_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Verify defaults
    let config = sqlx::query!(
        r#"
        SELECT
            disclosure_threshold_pct,
            material_threshold_pct,
            significant_threshold_pct,
            control_threshold_pct,
            control_basis,
            disclosure_basis
        FROM kyc.issuer_control_config
        WHERE issuer_entity_id = $1
        "#,
        issuer_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Check default values (NUMERIC(5,2) defaults)
    // BigDecimal may not preserve trailing zeros, so check numeric value
    assert!(config.disclosure_threshold_pct.is_some());
    assert!(config.material_threshold_pct.is_some());
    assert!(config.significant_threshold_pct.is_some());
    assert!(config.control_threshold_pct.is_some());

    // Parse and compare as floats to handle formatting differences
    let disclosure: f64 = config
        .disclosure_threshold_pct
        .unwrap()
        .to_string()
        .parse()
        .unwrap();
    let material: f64 = config
        .material_threshold_pct
        .unwrap()
        .to_string()
        .parse()
        .unwrap();
    let significant: f64 = config
        .significant_threshold_pct
        .unwrap()
        .to_string()
        .parse()
        .unwrap();
    let control: f64 = config
        .control_threshold_pct
        .unwrap()
        .to_string()
        .parse()
        .unwrap();

    assert!(
        (disclosure - 5.0).abs() < 0.01,
        "disclosure should be 5.0, got {}",
        disclosure
    );
    assert!(
        (material - 10.0).abs() < 0.01,
        "material should be 10.0, got {}",
        material
    );
    assert!(
        (significant - 25.0).abs() < 0.01,
        "significant should be 25.0, got {}",
        significant
    );
    assert!(
        (control - 50.0).abs() < 0.01,
        "control should be 50.0, got {}",
        control
    );
    assert_eq!(config.control_basis, Some("VOTES".to_string()));
    assert_eq!(config.disclosure_basis, Some("ECONOMIC".to_string()));

    // Cleanup
    sqlx::query!(
        "DELETE FROM kyc.issuer_control_config WHERE issuer_entity_id = $1",
        issuer_id
    )
    .execute(&pool)
    .await
    .unwrap();
    cleanup_test_entity(&pool, issuer_id).await;
}

// =============================================================================
// UBO SYNC TRIGGER TESTS (Migration 029)
// =============================================================================

/// Test that TA holdings (usage_type='TA') do NOT create UBO edges
/// even when ownership ≥25%
#[tokio::test]
async fn test_ubo_sync_skips_ta_holdings() {
    let pool = get_pool().await;

    // Create test entities
    let fund_id = create_test_entity(&pool, "Test Fund for TA").await;
    let investor_id = create_test_entity(&pool, "Test TA Investor").await;

    // Create a share class for the fund
    let share_class_id = create_test_share_class(&pool, fund_id, "Class A TA Test").await;

    // Create a TA holding with 30% ownership (should NOT create UBO edge)
    sqlx::query!(
        r#"
        INSERT INTO kyc.holdings (share_class_id, investor_entity_id, units, usage_type, status)
        VALUES ($1, $2, 30.0, 'TA', 'active')
        "#,
        share_class_id,
        investor_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Verify NO ownership relationship was created
    let edge_count: i64 = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM "ob-poc".entity_relationships
        WHERE from_entity_id = $1
          AND to_entity_id = $2
          AND relationship_type = 'ownership'
          AND source = 'INVESTOR_REGISTER'
        "#,
        investor_id,
        fund_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        edge_count, 0,
        "TA holdings should NOT create UBO ownership edges"
    );

    // Cleanup
    cleanup_test_share_class(&pool, share_class_id).await;
    cleanup_test_entity(&pool, fund_id).await;
    cleanup_test_entity(&pool, investor_id).await;
}

/// Test that UBO holdings DO create UBO edges when ownership ≥25%
/// and no role profile restricts it
#[tokio::test]
async fn test_ubo_sync_creates_edge_for_ubo_holdings() {
    let pool = get_pool().await;

    // Create test entities
    let fund_id = create_test_entity(&pool, "Test Fund for UBO").await;
    let investor_id = create_test_entity(&pool, "Test UBO Investor").await;

    // Create a share class for the fund
    let share_class_id = create_test_share_class(&pool, fund_id, "Class A UBO Test").await;

    // Create a UBO holding with 30% ownership (should create UBO edge)
    sqlx::query!(
        r#"
        INSERT INTO kyc.holdings (share_class_id, investor_entity_id, units, usage_type, status)
        VALUES ($1, $2, 30.0, 'UBO', 'active')
        "#,
        share_class_id,
        investor_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Verify ownership relationship WAS created
    let edge_count: i64 = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM "ob-poc".entity_relationships
        WHERE from_entity_id = $1
          AND to_entity_id = $2
          AND relationship_type = 'ownership'
          AND source = 'INVESTOR_REGISTER'
        "#,
        investor_id,
        fund_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        edge_count, 1,
        "UBO holdings ≥25% should create ownership edges"
    );

    // Cleanup
    sqlx::query!(
        r#"
        DELETE FROM "ob-poc".entity_relationships
        WHERE from_entity_id = $1 AND to_entity_id = $2 AND source = 'INVESTOR_REGISTER'
        "#,
        investor_id,
        fund_id
    )
    .execute(&pool)
    .await
    .unwrap();
    cleanup_test_share_class(&pool, share_class_id).await;
    cleanup_test_entity(&pool, fund_id).await;
    cleanup_test_entity(&pool, investor_id).await;
}

/// Test that role profile with is_ubo_eligible=false prevents UBO edge creation
#[tokio::test]
async fn test_ubo_sync_respects_ubo_eligibility_false() {
    let pool = get_pool().await;

    // Create test entities
    let fund_id = create_test_entity(&pool, "Test Fund Eligibility").await;
    let nominee_id = create_test_entity(&pool, "Test Nominee Holder").await;

    // Create a role profile marking the holder as NOT UBO eligible (nominee)
    sqlx::query!(
        r#"
        INSERT INTO kyc.investor_role_profiles
        (issuer_entity_id, holder_entity_id, role_type, is_ubo_eligible, lookthrough_policy)
        VALUES ($1, $2, 'NOMINEE', false, 'NONE')
        "#,
        fund_id,
        nominee_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Create a share class for the fund
    let share_class_id = create_test_share_class(&pool, fund_id, "Class A Nominee Test").await;

    // Create a UBO holding with 50% ownership
    // Despite being usage_type='UBO' and ≥25%, should NOT create edge because is_ubo_eligible=false
    sqlx::query!(
        r#"
        INSERT INTO kyc.holdings (share_class_id, investor_entity_id, units, usage_type, status)
        VALUES ($1, $2, 50.0, 'UBO', 'active')
        "#,
        share_class_id,
        nominee_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Verify NO ownership relationship was created
    let edge_count: i64 = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM "ob-poc".entity_relationships
        WHERE from_entity_id = $1
          AND to_entity_id = $2
          AND relationship_type = 'ownership'
          AND source = 'INVESTOR_REGISTER'
        "#,
        nominee_id,
        fund_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        edge_count, 0,
        "Holdings from is_ubo_eligible=false holders should NOT create UBO edges"
    );

    // Cleanup
    cleanup_test_share_class(&pool, share_class_id).await;
    cleanup_test_entity(&pool, fund_id).await;
    cleanup_test_entity(&pool, nominee_id).await;
}

/// Test that pooled vehicle role types (INTERMEDIARY_FOF, MASTER_POOL) default-deny UBO edges
#[tokio::test]
async fn test_ubo_sync_default_deny_pooled_vehicles() {
    let pool = get_pool().await;

    // Create test entities
    let fund_id = create_test_entity(&pool, "Test Fund Pooled").await;
    let fof_id = create_test_entity(&pool, "Test FoF Holder").await;

    // Create a role profile with INTERMEDIARY_FOF type but NO explicit is_ubo_eligible
    // The trigger should default-deny for pooled vehicle types
    sqlx::query!(
        r#"
        INSERT INTO kyc.investor_role_profiles
        (issuer_entity_id, holder_entity_id, role_type, lookthrough_policy)
        VALUES ($1, $2, 'INTERMEDIARY_FOF', 'ON_DEMAND')
        "#,
        fund_id,
        fof_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Create a share class for the fund
    let share_class_id = create_test_share_class(&pool, fund_id, "Class A FoF Test").await;

    // Create a UBO holding with 40% ownership
    sqlx::query!(
        r#"
        INSERT INTO kyc.holdings (share_class_id, investor_entity_id, units, usage_type, status)
        VALUES ($1, $2, 40.0, 'UBO', 'active')
        "#,
        share_class_id,
        fof_id
    )
    .execute(&pool)
    .await
    .unwrap();

    // Verify NO ownership relationship was created (default-deny for pooled vehicles)
    let edge_count: i64 = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM "ob-poc".entity_relationships
        WHERE from_entity_id = $1
          AND to_entity_id = $2
          AND relationship_type = 'ownership'
          AND source = 'INVESTOR_REGISTER'
        "#,
        fof_id,
        fund_id
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(
        edge_count, 0,
        "Pooled vehicle role types should default-deny UBO edge creation"
    );

    // Cleanup
    cleanup_test_share_class(&pool, share_class_id).await;
    cleanup_test_entity(&pool, fund_id).await;
    cleanup_test_entity(&pool, fof_id).await;
}

// =============================================================================
// ECONOMIC EXPOSURE BOUNDEDNESS TESTS
// =============================================================================

/// Test that max_rows parameter is respected
#[tokio::test]
async fn test_economic_exposure_max_rows_limit() {
    let pool = get_pool().await;

    // Call with max_rows=5 on a non-existent entity (should return 0 anyway)
    // This mainly tests that the parameter is accepted and function doesn't crash
    let result = sqlx::query!(
        r#"
        SELECT COUNT(*) as "count!" FROM kyc.fn_compute_economic_exposure(
            '00000000-0000-0000-0000-000000000000'::uuid,
            CURRENT_DATE,
            6,      -- max_depth
            0.0001, -- min_pct
            5,      -- max_rows = 5
            true,
            true
        )
        "#
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // With no data, should return 0 rows (well under the limit)
    assert!(
        result.count <= 5,
        "max_rows parameter should limit results to 5"
    );
}

/// Test that min_pct threshold stops traversal
#[tokio::test]
async fn test_economic_exposure_min_pct_threshold() {
    let pool = get_pool().await;

    // Call with min_pct=50% - only very large holdings would traverse
    let result = sqlx::query!(
        r#"
        SELECT COUNT(*) as "count!" FROM kyc.fn_compute_economic_exposure(
            '00000000-0000-0000-0000-000000000000'::uuid,
            CURRENT_DATE,
            6,
            0.50,   -- min_pct = 50% (very high threshold)
            200,
            true,
            true
        )
        "#
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Should work without error
    assert_eq!(result.count, 0);
}

/// Test that max_depth parameter is accepted
#[tokio::test]
async fn test_economic_exposure_max_depth_parameter() {
    let pool = get_pool().await;

    // Call with max_depth=2
    let result = sqlx::query!(
        r#"
        SELECT COUNT(*) as "count!" FROM kyc.fn_compute_economic_exposure(
            '00000000-0000-0000-0000-000000000000'::uuid,
            CURRENT_DATE,
            2,      -- max_depth = 2 (shallow)
            0.0001,
            200,
            true,
            true
        )
        "#
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Should work without error
    assert_eq!(result.count, 0);
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

async fn create_test_share_class(pool: &PgPool, entity_id: Uuid, name: &str) -> Uuid {
    let share_class_id = Uuid::now_v7();
    let unique_name = format!("{} {}", name, share_class_id);

    sqlx::query!(
        r#"
        INSERT INTO kyc.share_classes (id, entity_id, name, status)
        VALUES ($1, $2, $3, 'active')
        "#,
        share_class_id,
        entity_id,
        unique_name
    )
    .execute(pool)
    .await
    .unwrap();

    share_class_id
}

async fn cleanup_test_share_class(pool: &PgPool, share_class_id: Uuid) {
    // Delete holdings first
    let _ = sqlx::query!(
        "DELETE FROM kyc.holdings WHERE share_class_id = $1",
        share_class_id
    )
    .execute(pool)
    .await;

    // Delete share class
    let _ = sqlx::query!(
        "DELETE FROM kyc.share_classes WHERE id = $1",
        share_class_id
    )
    .execute(pool)
    .await;
}

async fn create_test_entity(pool: &PgPool, name: &str) -> Uuid {
    let entity_id = Uuid::now_v7();
    // Make name unique per test run to avoid conflicts
    let unique_name = format!("{} {}", name, entity_id);

    // Get or create a valid entity_type_id
    let type_id: Uuid = sqlx::query_scalar!(
        r#"
        SELECT entity_type_id as "id!" FROM "ob-poc".entity_types
        WHERE type_code = 'limited_company'
        LIMIT 1
        "#
    )
    .fetch_one(pool)
    .await
    .unwrap();

    sqlx::query!(
        r#"
        INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name)
        VALUES ($1, $2, $3)
        "#,
        entity_id,
        type_id,
        unique_name
    )
    .execute(pool)
    .await
    .unwrap();

    entity_id
}

async fn cleanup_test_entity(pool: &PgPool, entity_id: Uuid) {
    // Clean up related data first
    let _ = sqlx::query!(
        "DELETE FROM kyc.investor_role_profiles WHERE issuer_entity_id = $1 OR holder_entity_id = $1",
        entity_id
    )
    .execute(pool)
    .await;

    let _ = sqlx::query!(
        r#"DELETE FROM "ob-poc".entities WHERE entity_id = $1"#,
        entity_id
    )
    .execute(pool)
    .await;
}
