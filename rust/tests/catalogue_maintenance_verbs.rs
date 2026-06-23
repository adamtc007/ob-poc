#![cfg(feature = "database")]

use std::sync::Arc;

use anyhow::{anyhow, Result};
use dsl_runtime::{
    AttributeService, ServiceRegistryBuilder, TransactionScope, VerbExecutionContext,
    VerbExecutionOutcome,
};
use ob_poc::sem_reg::types::ObjectType;
use ob_poc::sequencer_tx::PgTransactionScope;
use ob_poc::services::ObPocAttributeService;
use sem_os_core::principal::Principal;
use sem_os_postgres::ops::{build_registry, SemOsVerbOpRegistry};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

async fn test_pool() -> Result<PgPool> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".into());
    Ok(PgPool::connect(&database_url).await?)
}

fn registry() -> SemOsVerbOpRegistry {
    let mut registry = build_registry();
    ob_poc::domain_ops::extend_registry(&mut registry);
    registry
}

fn attribute_services() -> Arc<dsl_runtime::ServiceRegistry> {
    let mut builder = ServiceRegistryBuilder::new();
    builder.register::<dyn AttributeService>(Arc::new(ObPocAttributeService::new()));
    Arc::new(builder.build())
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

fn outcome_uuid(outcome: VerbExecutionOutcome, fqn: &str) -> Result<Uuid> {
    match outcome {
        VerbExecutionOutcome::Uuid(id) => Ok(id),
        VerbExecutionOutcome::Record(record) => record
            .get("id")
            .or_else(|| record.get("product_id"))
            .or_else(|| record.get("resource_id"))
            .or_else(|| record.get("capability_id"))
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("{fqn} did not return a UUID record field"))
            .and_then(|raw| Ok(Uuid::parse_str(raw)?)),
        VerbExecutionOutcome::RecordSet(records) => Err(anyhow!(
            "{fqn} returned {} records, expected UUID",
            records.len()
        )),
        VerbExecutionOutcome::Affected(rows) => {
            Err(anyhow!("{fqn} affected {rows} rows, expected UUID"))
        }
        VerbExecutionOutcome::Void => Err(anyhow!("{fqn} returned void, expected UUID")),
    }
}

fn outcome_affected(outcome: VerbExecutionOutcome, fqn: &str) -> Result<u64> {
    match outcome {
        VerbExecutionOutcome::Affected(rows) => Ok(rows),
        VerbExecutionOutcome::Uuid(id) => {
            Err(anyhow!("{fqn} returned UUID {id}, expected affected"))
        }
        VerbExecutionOutcome::Record(record) => Err(anyhow!(
            "{fqn} returned record {record:?}, expected affected"
        )),
        VerbExecutionOutcome::RecordSet(records) => Err(anyhow!(
            "{fqn} returned {} records, expected affected",
            records.len()
        )),
        VerbExecutionOutcome::Void => Err(anyhow!("{fqn} returned void, expected affected")),
    }
}

async fn cleanup_test_attribute(pool: &PgPool, semantic_id: &str) -> Result<()> {
    let object_id = ob_poc::sem_reg::ids::object_id_for(ObjectType::AttributeDef, semantic_id);
    sqlx::query(r#"DELETE FROM "ob-poc".attribute_registry WHERE id = $1"#)
        .bind(semantic_id)
        .execute(pool)
        .await?;
    sqlx::query(
        r#"
        DELETE FROM sem_reg.snapshots
        WHERE object_type = 'attribute_def'::sem_reg.object_type
          AND object_id = $1
        "#,
    )
    .bind(object_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn catalogue_maintenance_verbs_write_and_read_back() -> Result<()> {
    let pool = test_pool().await?;
    let registry = registry();
    let mut scope = PgTransactionScope::begin(&pool).await?;
    let principal = Principal::in_process(
        "catalogue-maintenance-test",
        vec!["resource_owner".to_string(), "compliance_admin".to_string()],
    );
    let mut ctx = VerbExecutionContext::new(principal);
    let token_source = Uuid::new_v4().simple().to_string();
    let token = &token_source[..12];

    let service_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT service_id
        FROM "ob-poc".services
        ORDER BY (service_code = 'SETTLEMENT') DESC, service_code NULLS LAST
        LIMIT 1
        "#,
    )
    .fetch_one(scope.executor())
    .await?;

    let owner_system = format!("CODX_OWNER_{token}");
    let owner_fqn = format!("resource_owner:{owner_system}");
    let owner_record = execute(
        &registry,
        "resource-owner.assign",
        json!({
            "owner-system": owner_system,
            "owner-principal-fqn": owner_fqn,
            "display-name": format!("Codex owner {token}"),
            "dispatch-enabled": true
        }),
        &mut ctx,
        &mut scope,
    )
    .await?;
    assert!(matches!(owner_record, VerbExecutionOutcome::Record(_)));

    let owner_status: String = sqlx::query_scalar(
        r#"SELECT status FROM "ob-poc".resource_owner_principals WHERE owner_principal_fqn = $1"#,
    )
    .bind(&owner_fqn)
    .fetch_one(scope.executor())
    .await?;
    assert_eq!(owner_status, "active");

    let product_code = format!("CODX_PROD_{token}");
    let product_id = outcome_uuid(
        execute(
            &registry,
            "product.define",
            json!({
                "name": format!("Codex product {token}"),
                "product-code": product_code,
                "product-category": "test",
                "owner-principal-fqn": owner_fqn,
                "governance-status": "draft",
                "metadata": {"source": "catalogue_maintenance_verbs"}
            }),
            &mut ctx,
            &mut scope,
        )
        .await?,
        "product.define",
    )?;

    let product_row: (String, String) = sqlx::query_as(
        r#"SELECT product_code, governance_status FROM "ob-poc".products WHERE product_id = $1"#,
    )
    .bind(product_id)
    .fetch_one(scope.executor())
    .await?;
    assert_eq!(product_row, (product_code.clone(), "draft".to_string()));

    assert_eq!(
        outcome_affected(
            execute(
                &registry,
                "product.amend",
                json!({
                    "product-id": product_id,
                    "description": "amended product description",
                    "product-category": "test-amended",
                    "governance-status": "active"
                }),
                &mut ctx,
                &mut scope,
            )
            .await?,
            "product.amend",
        )?,
        1
    );
    let product_category: String = sqlx::query_scalar(
        r#"SELECT product_category FROM "ob-poc".products WHERE product_id = $1"#,
    )
    .bind(product_id)
    .fetch_one(scope.executor())
    .await?;
    assert_eq!(product_category, "test-amended");

    assert!(matches!(
        execute(
            &registry,
            "product-service.link",
            json!({
                "product-id": product_id,
                "service-id": service_id,
                "is-mandatory": true,
                "display-order": 17,
                "configuration": {"mode": "initial"}
            }),
            &mut ctx,
            &mut scope,
        )
        .await?,
        VerbExecutionOutcome::Record(_)
    ));
    let link_row: (bool, Option<i32>) = sqlx::query_as(
        r#"
        SELECT is_mandatory, display_order
        FROM "ob-poc".product_services
        WHERE product_id = $1 AND service_id = $2
        "#,
    )
    .bind(product_id)
    .bind(service_id)
    .fetch_one(scope.executor())
    .await?;
    assert_eq!(link_row, (true, Some(17)));

    assert_eq!(
        outcome_affected(
            execute(
                &registry,
                "product-service.amend",
                json!({
                    "product-id": product_id,
                    "service-id": service_id,
                    "is-default": true,
                    "display-order": 18,
                    "configuration": {"mode": "amended"}
                }),
                &mut ctx,
                &mut scope,
            )
            .await?,
            "product-service.amend",
        )?,
        1
    );
    let display_order: Option<i32> = sqlx::query_scalar(
        r#"
        SELECT display_order
        FROM "ob-poc".product_services
        WHERE product_id = $1 AND service_id = $2
        "#,
    )
    .bind(product_id)
    .bind(service_id)
    .fetch_one(scope.executor())
    .await?;
    assert_eq!(display_order, Some(18));

    let resource_code = format!("CODX_RES_{token}");
    let resource_id = outcome_uuid(
        execute(
            &registry,
            "service-resource.define-type",
            json!({
                "name": format!("Codex resource {token}"),
                "resource-code": resource_code,
                "resource-type": "application",
                "owner": owner_system,
                "owner-principal-fqn": owner_fqn,
                "description": "test resource",
                "governance-status": "draft"
            }),
            &mut ctx,
            &mut scope,
        )
        .await?,
        "service-resource.define-type",
    )?;
    let resource_row: (String, String) = sqlx::query_as(
        r#"
        SELECT resource_code, governance_status
        FROM "ob-poc".service_resource_types
        WHERE resource_id = $1
        "#,
    )
    .bind(resource_id)
    .fetch_one(scope.executor())
    .await?;
    assert_eq!(resource_row, (resource_code.clone(), "draft".to_string()));

    assert_eq!(
        outcome_affected(
            execute(
                &registry,
                "service-resource.amend-type",
                json!({
                    "resource-id": resource_id,
                    "description": "amended resource",
                    "lifecycle-status": "active",
                    "governance-status": "active",
                    "metadata": {"phase": "unit3"}
                }),
                &mut ctx,
                &mut scope,
            )
            .await?,
            "service-resource.amend-type",
        )?,
        1
    );
    let resource_description: Option<String> = sqlx::query_scalar(
        r#"SELECT description FROM "ob-poc".service_resource_types WHERE resource_id = $1"#,
    )
    .bind(resource_id)
    .fetch_one(scope.executor())
    .await?;
    assert_eq!(resource_description.as_deref(), Some("amended resource"));

    let capability_id = outcome_uuid(
        execute(
            &registry,
            "service-resource.add-capability",
            json!({
                "service-id": service_id,
                "resource-id": resource_id,
                "supported-options": [{"key": "mode", "value": "initial"}],
                "priority": 10,
                "cost-factor": "1.25",
                "performance-rating": 3,
                "is-required": false
            }),
            &mut ctx,
            &mut scope,
        )
        .await?,
        "service-resource.add-capability",
    )?;
    let capability_row: (i32, bool) = sqlx::query_as(
        r#"
        SELECT priority, is_active
        FROM "ob-poc".service_resource_capabilities
        WHERE capability_id = $1
        "#,
    )
    .bind(capability_id)
    .fetch_one(scope.executor())
    .await?;
    assert_eq!(capability_row, (10, true));

    assert_eq!(
        outcome_affected(
            execute(
                &registry,
                "service-resource.amend-capability",
                json!({
                    "capability-id": capability_id,
                    "priority": 42,
                    "performance-rating": 4,
                    "is-required": true,
                    "resource-config": {"mode": "amended"}
                }),
                &mut ctx,
                &mut scope,
            )
            .await?,
            "service-resource.amend-capability",
        )?,
        1
    );
    let capability_priority: i32 = sqlx::query_scalar(
        r#"SELECT priority FROM "ob-poc".service_resource_capabilities WHERE capability_id = $1"#,
    )
    .bind(capability_id)
    .fetch_one(scope.executor())
    .await?;
    assert_eq!(capability_priority, 42);

    assert_eq!(
        outcome_affected(
            execute(
                &registry,
                "service-resource.remove-capability",
                json!({"capability-id": capability_id}),
                &mut ctx,
                &mut scope,
            )
            .await?,
            "service-resource.remove-capability",
        )?,
        1
    );
    let capability_active: bool = sqlx::query_scalar(
        r#"SELECT is_active FROM "ob-poc".service_resource_capabilities WHERE capability_id = $1"#,
    )
    .bind(capability_id)
    .fetch_one(scope.executor())
    .await?;
    assert!(!capability_active);

    assert_eq!(
        outcome_affected(
            execute(
                &registry,
                "product-service.unlink",
                json!({"product-id": product_id, "service-id": service_id}),
                &mut ctx,
                &mut scope,
            )
            .await?,
            "product-service.unlink",
        )?,
        1
    );
    let link_count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM "ob-poc".product_services
        WHERE product_id = $1 AND service_id = $2
        "#,
    )
    .bind(product_id)
    .bind(service_id)
    .fetch_one(scope.executor())
    .await?;
    assert_eq!(link_count, 0);

    assert_eq!(
        outcome_affected(
            execute(
                &registry,
                "product.retire",
                json!({"product-id": product_id}),
                &mut ctx,
                &mut scope,
            )
            .await?,
            "product.retire",
        )?,
        1
    );
    let product_active: bool =
        sqlx::query_scalar(r#"SELECT is_active FROM "ob-poc".products WHERE product_id = $1"#)
            .bind(product_id)
            .fetch_one(scope.executor())
            .await?;
    assert!(!product_active);

    assert_eq!(
        outcome_affected(
            execute(
                &registry,
                "service-resource.retire-type",
                json!({"resource-id": resource_id}),
                &mut ctx,
                &mut scope,
            )
            .await?,
            "service-resource.retire-type",
        )?,
        1
    );
    let resource_active: bool = sqlx::query_scalar(
        r#"SELECT is_active FROM "ob-poc".service_resource_types WHERE resource_id = $1"#,
    )
    .bind(resource_id)
    .fetch_one(scope.executor())
    .await?;
    assert!(!resource_active);

    assert_eq!(
        outcome_affected(
            execute(
                &registry,
                "resource-owner.amend",
                json!({
                    "owner-principal-fqn": owner_fqn,
                    "display-name": format!("Codex owner amended {token}"),
                    "metadata": {"phase": "unit3"}
                }),
                &mut ctx,
                &mut scope,
            )
            .await?,
            "resource-owner.amend",
        )?,
        1
    );
    let owner_name: Option<String> = sqlx::query_scalar(
        r#"SELECT display_name FROM "ob-poc".resource_owner_principals WHERE owner_principal_fqn = $1"#,
    )
    .bind(&owner_fqn)
    .fetch_one(scope.executor())
    .await?;
    assert_eq!(owner_name, Some(format!("Codex owner amended {token}")));

    assert_eq!(
        outcome_affected(
            execute(
                &registry,
                "resource-owner.unassign",
                json!({"owner-principal-fqn": owner_fqn}),
                &mut ctx,
                &mut scope,
            )
            .await?,
            "resource-owner.unassign",
        )?,
        1
    );
    let owner_retired: (String, bool) = sqlx::query_as(
        r#"
        SELECT status, dispatch_enabled
        FROM "ob-poc".resource_owner_principals
        WHERE owner_principal_fqn = $1
        "#,
    )
    .bind(&owner_fqn)
    .fetch_one(scope.executor())
    .await?;
    assert_eq!(owner_retired, ("retired".to_string(), false));

    drop(scope);
    Ok(())
}

#[tokio::test]
async fn catalogue_maintenance_verbs_require_catalogue_authority() -> Result<()> {
    let pool = test_pool().await?;
    let registry = registry();
    let mut scope = PgTransactionScope::begin(&pool).await?;
    let principal = Principal::in_process("catalogue-denied-test", vec!["analyst".to_string()]);
    let mut ctx = VerbExecutionContext::new(principal);

    for fqn in [
        "product.define",
        "product.amend",
        "product.retire",
        "service.define",
        "service-version.draft",
        "service-resource.define-type",
        "service-resource.amend-type",
        "service-resource.retire-type",
        "product-service.link",
        "product-service.amend",
        "product-service.unlink",
        "service-resource.add-capability",
        "service-resource.amend-capability",
        "service-resource.remove-capability",
        "resource-owner.assign",
        "resource-owner.amend",
        "resource-owner.unassign",
    ] {
        let err = execute(&registry, fqn, json!({}), &mut ctx, &mut scope)
            .await
            .expect_err(fqn);
        assert!(
            err.to_string().contains("requires one of roles"),
            "{fqn}: {err}"
        );
    }

    drop(scope);
    Ok(())
}

#[tokio::test]
async fn resource_attribute_define_requires_resource_owner_authority() -> Result<()> {
    let pool = test_pool().await?;
    let registry = registry();
    let services = attribute_services();
    let token_source = Uuid::new_v4().simple().to_string();
    let token = &token_source[..12];
    let non_resource_id = format!("codex_unit5.compliance_{token}");
    let resource_id = format!("codex_unit5.resource_{token}");

    let mut denied_scope = PgTransactionScope::begin(&pool).await?;
    let mut denied_ctx = VerbExecutionContext::with_services(
        Principal::in_process("attribute-denied-test", vec!["analyst".to_string()]),
        services.clone(),
    );
    let denied = execute(
        &registry,
        "attribute.define",
        json!({
            "id": resource_id,
            "display-name": format!("Codex resource attribute {token}"),
            "category": "resource",
            "value-type": "string",
            "domain": "codex_unit5"
        }),
        &mut denied_ctx,
        &mut denied_scope,
    )
    .await
    .expect_err("resource category should require resource_owner authority");
    assert!(
        denied
            .to_string()
            .contains("category=resource requires one of roles"),
        "{denied}"
    );
    drop(denied_scope);

    let mut non_resource_scope = PgTransactionScope::begin(&pool).await?;
    let mut non_resource_ctx = VerbExecutionContext::with_services(
        Principal::in_process("attribute-non-resource-test", vec!["analyst".to_string()]),
        services.clone(),
    );
    let non_resource_outcome = execute(
        &registry,
        "attribute.define",
        json!({
            "id": non_resource_id,
            "display-name": format!("Codex compliance attribute {token}"),
            "category": "compliance",
            "value-type": "string",
            "domain": "codex_unit5"
        }),
        &mut non_resource_ctx,
        &mut non_resource_scope,
    )
    .await?;
    assert!(matches!(
        non_resource_outcome,
        VerbExecutionOutcome::Uuid(_)
    ));
    drop(non_resource_scope);

    let mut allowed_scope = PgTransactionScope::begin(&pool).await?;
    let mut allowed_ctx = VerbExecutionContext::with_services(
        Principal::in_process(
            "attribute-resource-owner-test",
            vec!["resource_owner".to_string()],
        ),
        services,
    );
    let allowed_outcome = execute(
        &registry,
        "attribute.define",
        json!({
            "id": resource_id,
            "display-name": format!("Codex resource attribute {token}"),
            "category": "resource",
            "value-type": "string",
            "domain": "codex_unit5"
        }),
        &mut allowed_ctx,
        &mut allowed_scope,
    )
    .await?;
    assert!(matches!(allowed_outcome, VerbExecutionOutcome::Uuid(_)));
    drop(allowed_scope);

    cleanup_test_attribute(&pool, &non_resource_id).await?;
    cleanup_test_attribute(&pool, &resource_id).await?;
    Ok(())
}
