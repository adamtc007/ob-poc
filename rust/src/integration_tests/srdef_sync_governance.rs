use anyhow::{Context, Result};
use sqlx::PgPool;
use uuid::Uuid;

use crate::sem_reg::ids::object_id_for;
use crate::sem_reg::ObjectType;
use crate::service_resources::srdef_loader::{
    LoadedSrdef, LoadedSrdefAttribute, SrdefLoader, SrdefRegistry,
};

async fn cleanup(pool: &PgPool, srdef_id: &str, owner: &str) -> Result<()> {
    let resource_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT resource_id FROM "ob-poc".service_resource_types WHERE srdef_id = $1"#,
    )
    .bind(srdef_id)
    .fetch_all(pool)
    .await?;
    if !resource_ids.is_empty() {
        sqlx::query(
            r#"DELETE FROM "ob-poc".resource_attribute_requirements WHERE resource_id = ANY($1)"#,
        )
        .bind(&resource_ids)
        .execute(pool)
        .await?;
    }
    sqlx::query(r#"DELETE FROM "ob-poc".service_resource_types WHERE srdef_id = $1"#)
        .bind(srdef_id)
        .execute(pool)
        .await?;
    sqlx::query(r#"DELETE FROM "ob-poc".resource_owner_principals WHERE owner_principal_fqn = $1"#)
        .bind(format!("resource_owner:{owner}"))
        .execute(pool)
        .await?;
    let object_id = object_id_for(ObjectType::ServiceResourceDef, srdef_id);
    sqlx::query("DELETE FROM sem_reg.snapshots WHERE object_id = $1")
        .bind(object_id)
        .execute(pool)
        .await?;
    Ok(())
}

async fn pool() -> Result<PgPool> {
    let url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string());
    PgPool::connect(&url)
        .await
        .with_context(|| format!("connect DATABASE_URL={url}"))
}

async fn any_attribute_id(pool: &PgPool) -> Result<String> {
    sqlx::query_scalar(
        r#"SELECT id FROM "ob-poc".attribute_registry WHERE id IS NOT NULL ORDER BY id LIMIT 1"#,
    )
    .fetch_optional(pool)
    .await?
    .context("attribute_registry must contain at least one attribute")
}

fn registry(srdef: LoadedSrdef) -> SrdefRegistry {
    let mut registry = SrdefRegistry::new();
    registry.srdefs.insert(srdef.srdef_id.clone(), srdef);
    registry
}

#[tokio::test]
async fn sync_definitions_records_only_real_entity_changes() -> Result<()> {
    let pool = pool().await?;
    let attr_id = any_attribute_id(&pool).await?;
    let unique = Uuid::new_v4().simple().to_string();
    let owner = format!("PHASE15_{unique}");
    let code = format!("phase15_{unique}");
    let srdef_id = format!("SRDEF::{owner}::Account::{code}");
    cleanup(&pool, &srdef_id, &owner).await?;

    let loader = SrdefLoader::new("unused");
    let base = LoadedSrdef {
        srdef_id: srdef_id.clone(),
        code,
        name: "Phase 1.5 Sync Test Account".to_string(),
        resource_type: "Account".to_string(),
        purpose: Some("Phase 1.5 sync idempotency test".to_string()),
        provisioning_strategy: "request".to_string(),
        owner: owner.clone(),
        triggered_by_services: Vec::new(),
        attributes: vec![LoadedSrdefAttribute {
            attr_id,
            requirement: "required".to_string(),
            source_policy: vec!["manual".to_string()],
            constraints: serde_json::json!({}),
            evidence_policy: serde_json::json!({}),
            default_value: None,
            condition: None,
            description: Some("Phase 1.5 test attribute".to_string()),
        }],
        depends_on: Vec::new(),
        per_market: false,
        per_currency: false,
        per_counterparty: false,
        application_binding: None,
    };

    let first = loader
        .sync_to_database(&pool, &registry(base.clone()))
        .await?;
    assert!(
        first.recorded_transitions() >= 3,
        "initial load should record owner, SRDEF, and attribute transitions: {first:?}"
    );

    let second = loader
        .sync_to_database(&pool, &registry(base.clone()))
        .await?;
    eprintln!(
        "[phase1.5] idempotent reload recorded_transitions={}",
        second.recorded_transitions()
    );
    assert_eq!(
        second.recorded_transitions(),
        0,
        "unchanged sync must record zero transitions: {second:?}"
    );

    let mut changed = base;
    changed.name = "Phase 1.5 Sync Test Account Amended".to_string();
    let third = loader.sync_to_database(&pool, &registry(changed)).await?;
    eprintln!(
        "[phase1.5] one-SRDEF change recorded_transitions={}",
        third.recorded_transitions()
    );
    assert_eq!(
        third.recorded_transitions(),
        1,
        "one changed SRDEF should record exactly one entity transition: {third:?}"
    );

    cleanup(&pool, &srdef_id, &owner).await?;
    Ok(())
}
