#[cfg(feature = "database")]
mod tests {
    use std::collections::HashMap;

    use anyhow::Result;
    use ob_poc::database::{CbuService, EntityService};
    use ob_poc::dsl_v2::execution::{
        runtime_registry, DslExecutor, ExecutionContext, GenericCrudExecutor,
        GenericExecutionResult,
    };
    use ob_poc::dsl_v2::planning::compile;
    use ob_poc::dsl_v2::syntax::parse_program;
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::Uuid;

    async fn test_pool() -> Result<PgPool> {
        let database_url =
            std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".into());
        Ok(PgPool::connect(&database_url).await?)
    }

    #[tokio::test]
    async fn cbu_delete_sets_deleted_at_and_hides_row() -> Result<()> {
        let pool = test_pool().await?;
        let executor = GenericCrudExecutor::new(pool.clone());
        let cbu_service = CbuService::new(pool.clone());

        let cbu_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, status, created_at, updated_at)
            VALUES ($1, $2, 'GB', 'DISCOVERED', NOW(), NOW())
            "#,
        )
        .bind(cbu_id)
        .bind(format!("soft-delete-cbu-{cbu_id}"))
        .execute(&pool)
        .await?;

        let runtime_verb = runtime_registry()
            .get("cbu", "delete")
            .expect("cbu.delete must exist");
        let args = HashMap::from([("cbu-id".to_string(), json!(cbu_id.to_string()))]);

        let result = executor.execute(runtime_verb, &args).await?;
        assert!(matches!(result, GenericExecutionResult::Affected(1)));

        let deleted_at_set: bool = sqlx::query_scalar(
            r#"SELECT deleted_at IS NOT NULL FROM "ob-poc".cbus WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .fetch_one(&pool)
        .await?;
        assert!(deleted_at_set);
        assert!(cbu_service.get_cbu_by_id(cbu_id).await?.is_none());

        sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#)
            .bind(cbu_id)
            .execute(&pool)
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn entity_delete_sets_deleted_at_and_hides_row() -> Result<()> {
        let pool = test_pool().await?;
        let executor = GenericCrudExecutor::new(pool.clone());
        let entity_service = EntityService::new(pool.clone());

        let entity_id = Uuid::new_v4();
        let entity_type_id: Uuid = sqlx::query_scalar(
            r#"SELECT entity_type_id FROM "ob-poc".entity_types ORDER BY entity_type_id LIMIT 1"#,
        )
        .fetch_one(&pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name, created_at, updated_at)
            VALUES ($1, $2, $3, NOW(), NOW())
            "#,
        )
        .bind(entity_id)
        .bind(entity_type_id)
        .bind(format!("soft-delete-entity-{entity_id}"))
        .execute(&pool)
        .await?;

        let runtime_verb = runtime_registry()
            .get("entity", "delete")
            .expect("entity.delete must exist");
        let args = HashMap::from([("entity-id".to_string(), json!(entity_id.to_string()))]);

        let result = executor.execute(runtime_verb, &args).await?;
        assert!(matches!(result, GenericExecutionResult::Affected(1)));

        let deleted_at_set: bool = sqlx::query_scalar(
            r#"SELECT deleted_at IS NOT NULL FROM "ob-poc".entities WHERE entity_id = $1"#,
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await?;
        assert!(deleted_at_set);
        assert!(entity_service.get_entity_by_id(entity_id).await?.is_none());

        sqlx::query(r#"DELETE FROM "ob-poc".entities WHERE entity_id = $1"#)
            .bind(entity_id)
            .execute(&pool)
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn cbu_delete_cascade_soft_deletes_root_rows() -> Result<()> {
        let pool = test_pool().await?;
        let executor = DslExecutor::new(pool.clone());
        let mut ctx = ExecutionContext::new();

        let cbu_id = Uuid::new_v4();
        let entity_id = Uuid::new_v4();
        let entity_type_id: Uuid = sqlx::query_scalar(
            r#"SELECT entity_type_id FROM "ob-poc".entity_types ORDER BY entity_type_id LIMIT 1"#,
        )
        .fetch_one(&pool)
        .await?;
        let role_id: Uuid =
            sqlx::query_scalar(r#"SELECT role_id FROM "ob-poc".roles ORDER BY role_id LIMIT 1"#)
                .fetch_one(&pool)
                .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, status, created_at, updated_at)
            VALUES ($1, $2, 'GB', 'DISCOVERED', NOW(), NOW())
            "#,
        )
        .bind(cbu_id)
        .bind(format!("soft-delete-cascade-cbu-{cbu_id}"))
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name, created_at, updated_at)
            VALUES ($1, $2, $3, NOW(), NOW())
            "#,
        )
        .bind(entity_id)
        .bind(entity_type_id)
        .bind(format!("soft-delete-cascade-entity-{entity_id}"))
        .execute(&pool)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbu_entity_roles (cbu_id, entity_id, role_id)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(cbu_id)
        .bind(entity_id)
        .bind(role_id)
        .execute(&pool)
        .await?;

        let dsl = format!("(cbu.delete-cascade :cbu-id \"{cbu_id}\")");
        let ast = parse_program(&dsl).map_err(|e| anyhow::anyhow!("{e:?}"))?;
        let plan = compile(&ast).map_err(|e| anyhow::anyhow!("{e:?}"))?;
        executor.execute_plan(&plan, &mut ctx).await?;

        let cbu_deleted_at_set: bool = sqlx::query_scalar(
            r#"SELECT deleted_at IS NOT NULL FROM "ob-poc".cbus WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .fetch_one(&pool)
        .await?;
        assert!(cbu_deleted_at_set);

        let entity_deleted_at_set: bool = sqlx::query_scalar(
            r#"SELECT deleted_at IS NOT NULL FROM "ob-poc".entities WHERE entity_id = $1"#,
        )
        .bind(entity_id)
        .fetch_one(&pool)
        .await?;
        assert!(entity_deleted_at_set);

        let history_rows: i64 = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*)::bigint
               FROM "ob-poc".cbu_entity_roles_history
               WHERE cbu_id = $1
                 AND entity_id = $2
                 AND operation = 'DELETE'"#,
        )
        .bind(cbu_id)
        .bind(entity_id)
        .fetch_one(&pool)
        .await?;
        assert!(history_rows >= 1);

        Ok(())
    }
}
