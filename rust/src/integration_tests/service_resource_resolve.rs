#![cfg(feature = "database")]

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{anyhow, ensure, Result};
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};
use crate::sequencer_tx::PgTransactionScope;
use crate::service_resources::resolve;
use sem_os_core::principal::Principal;
use sem_os_postgres::ops::{build_registry, SemOsVerbOpRegistry};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const WRITE_TABLES: &[&str] = &[
    "cbu_product_subscriptions",
    "service_delivery_map",
    "service_intents",
    "cbus",
    "srdef_discovery_reasons",
    "cbu_unified_attr_requirements",
    "cbu_attr_values",
    "provisioning_requests",
    "provisioning_events",
    "cbu_service_readiness",
];

async fn test_pool() -> Result<PgPool> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".into());
    Ok(PgPool::connect(&database_url).await?)
}

fn registry() -> SemOsVerbOpRegistry {
    let mut registry = build_registry();
    crate::domain_ops::extend_registry(&mut registry);
    registry
}

async fn execute(
    registry: &SemOsVerbOpRegistry,
    fqn: &str,
    args: Value,
    ctx: &mut VerbExecutionContext,
    scope: &mut PgTransactionScope,
) -> Result<VerbExecutionOutcome> {
    let op = registry
        .get(fqn)
        .ok_or_else(|| anyhow!("missing registered op {fqn}"))?;
    op.execute(&args, ctx, scope).await
}

async fn resolve_fixture(pool: &PgPool) -> Result<(Uuid, Uuid)> {
    let cbu_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT cbu_id
        FROM "ob-poc".cbus
        WHERE deleted_at IS NULL
        ORDER BY created_at NULLS LAST, cbu_id
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("no CBU fixture row exists"))?;

    let product_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT p.product_id
        FROM "ob-poc".products p
        WHERE p.product_code = 'CUSTODY'
          AND EXISTS (
              SELECT 1
              FROM "ob-poc".product_services ps
              WHERE ps.product_id = p.product_id
          )
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("CUSTODY product fixture with services does not exist"))?;

    Ok((cbu_id, product_id))
}

async fn cbu_profile_fixture_pair(pool: &PgPool) -> Result<(Uuid, Uuid)> {
    let matching: Uuid = sqlx::query_scalar(
        r#"
        SELECT cbu_id
        FROM "ob-poc".cbus
        WHERE deleted_at IS NULL
          AND jurisdiction = 'LU'
        ORDER BY created_at NULLS LAST, cbu_id
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("no LU CBU fixture row exists"))?;

    let non_matching: Uuid = sqlx::query_scalar(
        r#"
        SELECT cbu_id
        FROM "ob-poc".cbus
        WHERE deleted_at IS NULL
          AND jurisdiction IS NOT NULL
          AND jurisdiction <> 'LU'
        ORDER BY created_at NULLS LAST, cbu_id
        LIMIT 1
        "#,
    )
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| anyhow!("no non-LU CBU fixture row exists"))?;

    Ok((matching, non_matching))
}

async fn active_service_pair(pool: &PgPool) -> Result<(Uuid, Uuid)> {
    let rows: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT service_id
        FROM "ob-poc".services
        WHERE COALESCE(is_active, TRUE)
        ORDER BY (service_code = 'SETTLEMENT') DESC,
                 (service_code = 'NAV_CALC') DESC,
                 service_code NULLS LAST,
                 name,
                 service_id
        LIMIT 2
        "#,
    )
    .fetch_all(pool)
    .await?;

    ensure!(
        rows.len() >= 2,
        "profile-conditioned resolve test requires at least two active services"
    );
    Ok((rows[0], rows[1]))
}

async fn cleanup_profile_condition_fixture(pool: &PgPool, product_code: &str) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM "ob-poc".product_service_conditions
        WHERE product_id IN (
            SELECT product_id
            FROM "ob-poc".products
            WHERE product_code = $1
        )
        "#,
    )
    .bind(product_code)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        DELETE FROM "ob-poc".product_services
        WHERE product_id IN (
            SELECT product_id
            FROM "ob-poc".products
            WHERE product_code = $1
        )
        "#,
    )
    .bind(product_code)
    .execute(pool)
    .await?;

    sqlx::query(r#"DELETE FROM "ob-poc".products WHERE product_code = $1"#)
        .bind(product_code)
        .execute(pool)
        .await?;

    Ok(())
}

async fn base_service_ids(pool: &PgPool, product_id: Uuid) -> Result<BTreeSet<Uuid>> {
    let ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT service_id
        FROM "ob-poc".product_services
        WHERE product_id = $1
        "#,
    )
    .bind(product_id)
    .fetch_all(pool)
    .await?;

    Ok(ids.into_iter().collect())
}

fn resolved_service_ids(
    output: &crate::service_resources::ResolvedDependencies,
) -> BTreeSet<Uuid> {
    output
        .services
        .iter()
        .map(|service| service.service_id)
        .collect()
}

async fn write_table_counts(pool: &PgPool) -> Result<BTreeMap<&'static str, i64>> {
    let mut counts = BTreeMap::new();
    for table in WRITE_TABLES {
        let sql = format!(r#"SELECT COUNT(*)::bigint FROM "ob-poc".{table}"#);
        let count: i64 = sqlx::query_scalar(&sql).fetch_one(pool).await?;
        counts.insert(*table, count);
    }
    Ok(counts)
}

async fn cbu_discovery_state(pool: &PgPool, cbu_id: Uuid) -> Result<String> {
    sqlx::query_scalar(r#"SELECT cbu_discovery_state FROM "ob-poc".cbus WHERE cbu_id = $1"#)
        .bind(cbu_id)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

#[tokio::test]
async fn resolve_includes_profile_conditional_product_services() -> Result<()> {
    let pool = test_pool().await?;
    let (matching_cbu_id, non_matching_cbu_id) = cbu_profile_fixture_pair(&pool).await?;
    let (base_service_id, conditional_service_id) = active_service_pair(&pool).await?;
    let token_source = Uuid::new_v4().simple().to_string();
    let token = &token_source[..12];
    let product_id = Uuid::new_v4();
    let product_code = format!("CODX_RESOLVE_PROFILE_{token}");
    let condition_key = format!("codex.resolve.profile.{token}");

    cleanup_profile_condition_fixture(&pool, &product_code).await?;

    let registry = registry();
    let mut scope = PgTransactionScope::begin(&pool).await?;
    let principal = Principal::in_process(
        "resolve-profile-test",
        vec!["resource_owner".to_string(), "compliance_admin".to_string()],
    );
    let mut ctx = VerbExecutionContext::new(principal);

    execute(
        &registry,
        "product.define",
        json!({
            "product-id": product_id,
            "name": format!("Codex resolve profile {token}"),
            "product-code": product_code,
            "product-category": "test",
            "governance-status": "active",
            "metadata": {"source": "service_resource_resolve"}
        }),
        &mut ctx,
        &mut scope,
    )
    .await?;

    execute(
        &registry,
        "product-service.link",
        json!({
            "product-id": product_id,
            "service-id": base_service_id,
            "is-mandatory": true,
            "display-order": 1,
            "configuration": {"mode": "base"}
        }),
        &mut ctx,
        &mut scope,
    )
    .await?;

    execute(
        &registry,
        "product-service.link",
        json!({
            "product-id": product_id,
            "service-id": conditional_service_id,
            "condition-key": condition_key,
            "predicate-dsl": "jurisdiction = 'LU'",
            "description": "Codex profile-conditioned resolve test edge",
            "is-mandatory": true,
            "display-order": 2,
            "configuration": {"mode": "conditional"}
        }),
        &mut ctx,
        &mut scope,
    )
    .await?;
    scope.commit().await?;

    let result: Result<()> = async {
        let base_ids = base_service_ids(&pool, product_id).await?;
        let matching = resolve(&pool, matching_cbu_id, &[product_id]).await?;
        let non_matching = resolve(&pool, non_matching_cbu_id, &[product_id]).await?;
        let matching_ids = resolved_service_ids(&matching);
        let non_matching_ids = resolved_service_ids(&non_matching);

        ensure!(
            matching_ids.contains(&conditional_service_id),
            "matching CBU profile did not receive conditional service"
        );
        ensure!(
            !non_matching_ids.contains(&conditional_service_id),
            "non-matching CBU profile received conditional service"
        );
        ensure!(
            non_matching_ids == base_ids,
            "non-matching profile should resolve exactly the base product-service set"
        );
        ensure!(
            matching_ids != non_matching_ids,
            "different CBU profiles should produce different service sets"
        );

        Ok(())
    }
    .await;

    cleanup_profile_condition_fixture(&pool, &product_code).await?;
    result
}

#[tokio::test]
async fn resolve_is_deterministic_and_does_not_write_pipeline_tables() -> Result<()> {
    let pool = test_pool().await?;
    let (cbu_id, product_id) = resolve_fixture(&pool).await?;

    let counts_before = write_table_counts(&pool).await?;
    let state_before = cbu_discovery_state(&pool, cbu_id).await?;

    let first = resolve(&pool, cbu_id, &[product_id]).await?;
    let counts_after_first = write_table_counts(&pool).await?;
    let state_after_first = cbu_discovery_state(&pool, cbu_id).await?;

    let second = resolve(&pool, cbu_id, &[product_id]).await?;
    let counts_after_second = write_table_counts(&pool).await?;
    let state_after_second = cbu_discovery_state(&pool, cbu_id).await?;

    assert_eq!(first, second, "resolve output must be deterministic");
    assert!(
        !first.services.is_empty(),
        "fixture product must resolve services"
    );
    assert!(
        !first.resource_types.is_empty(),
        "fixture product must resolve SRDEF-backed resource types"
    );
    assert_eq!(
        counts_before, counts_after_first,
        "first resolve call changed write-side row counts"
    );
    assert_eq!(
        counts_before, counts_after_second,
        "second resolve call changed write-side row counts"
    );
    assert_eq!(
        state_before, state_after_first,
        "first resolve call changed cbu_discovery_state"
    );
    assert_eq!(
        state_before, state_after_second,
        "second resolve call changed cbu_discovery_state"
    );

    Ok(())
}
