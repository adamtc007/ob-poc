//! Service-options pilot catalogue smoke tests.

use sqlx::postgres::PgPoolOptions;

async fn pool() -> sqlx::PgPool {
    let url = std::env::var("TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .unwrap_or_else(|_| "postgresql:///data_designer".into());
    PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await
        .expect("connect to test database")
}

#[tokio::test]
async fn custody_settlement_options_and_rules_are_seeded() {
    let pool = pool().await;

    let option_count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM "ob-poc".service_option_defs od
        JOIN "ob-poc".services s USING (service_id)
        WHERE s.service_code = 'SETTLEMENT'
          AND od.lifecycle_status = 'active'
        "#,
    )
    .fetch_one(&pool)
    .await
    .expect("count settlement options");
    assert_eq!(option_count, 3);

    let fanout_count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM "ob-poc".service_resource_fanout_rules fr
        JOIN "ob-poc".services s USING (service_id)
        WHERE s.service_code = 'SETTLEMENT'
          AND fr.is_active
        "#,
    )
    .fetch_one(&pool)
    .await
    .expect("count settlement fanout rules");
    assert_eq!(fanout_count, 5);
}

#[tokio::test]
async fn fund_accounting_pilot_keeps_unknown_content_as_validation_gaps() {
    let pool = pool().await;

    let required_without_default: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM "ob-poc".service_option_defs od
        JOIN "ob-poc".services s USING (service_id)
        WHERE s.service_code IN ('NAV_CALC', 'FUND_REPORTING', 'ASSET_PRICING')
          AND od.lifecycle_status = 'active'
          AND od.is_required
          AND od.default_value IS NULL
        "#,
    )
    .fetch_one(&pool)
    .await
    .expect("count explicit gap-producing options");
    assert_eq!(required_without_default, 3);
}
