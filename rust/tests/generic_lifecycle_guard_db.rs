#[cfg(feature = "database")]
mod tests {
    use std::collections::HashMap;

    use anyhow::Result;
    use ob_poc::dsl_v2::generic_executor::{GenericCrudExecutor, GenericExecutionResult};
    use ob_poc::dsl_v2::runtime_registry::runtime_registry;
    use serde_json::json;
    use sqlx::PgPool;
    use uuid::Uuid;

    async fn test_pool() -> Result<PgPool> {
        let database_url =
            std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".into());
        Ok(PgPool::connect(&database_url).await?)
    }

    #[tokio::test]
    async fn rejects_invalid_entity_workstream_transition() -> Result<()> {
        let pool = test_pool().await?;
        let executor = GenericCrudExecutor::new(pool.clone());

        let (cbu_id, entity_id): (Uuid, Uuid) = sqlx::query_as(
            r#"
            SELECT c.cbu_id, e.entity_id
            FROM "ob-poc".cbus c
            CROSS JOIN "ob-poc".entities e
            LIMIT 1
            "#,
        )
        .fetch_one(&pool)
        .await?;

        let case_id = Uuid::new_v4();
        let workstream_id = Uuid::new_v4();

        let case_ref: String = sqlx::query_scalar(
            r#"
            INSERT INTO "ob-poc".cases (case_id, cbu_id, case_type)
            VALUES ($1, $2, 'NEW_CLIENT')
            RETURNING case_ref
            "#,
        )
        .bind(case_id)
        .bind(cbu_id)
        .fetch_one(&pool)
        .await?;
        assert!(!case_ref.is_empty());

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_workstreams (workstream_id, case_id, entity_id, status)
            VALUES ($1, $2, $3, 'PENDING')
            "#,
        )
        .bind(workstream_id)
        .bind(case_id)
        .bind(entity_id)
        .execute(&pool)
        .await?;

        let runtime_verb = runtime_registry()
            .get("entity-workstream", "update-status")
            .expect("entity-workstream.update-status must exist");

        let args = HashMap::from([
            (
                "workstream-id".to_string(),
                json!(workstream_id.to_string()),
            ),
            ("status".to_string(), json!("SCREEN")),
        ]);

        let error = executor
            .execute(runtime_verb, &args)
            .await
            .expect_err("invalid transition should be rejected");
        let message = error.to_string();
        assert!(
            message.contains("Invalid state transition for kyc_workstream")
                || message.contains("Invalid state transition for entity_workstream"),
            "unexpected error: {message}"
        );

        let current_status: String = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".entity_workstreams WHERE workstream_id = $1"#,
        )
        .bind(workstream_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(current_status, "PENDING");

        sqlx::query(r#"DELETE FROM "ob-poc".entity_workstreams WHERE workstream_id = $1"#)
            .bind(workstream_id)
            .execute(&pool)
            .await?;
        sqlx::query(r#"DELETE FROM "ob-poc".cases WHERE case_id = $1"#)
            .bind(case_id)
            .execute(&pool)
            .await?;

        Ok(())
    }

    #[tokio::test]
    async fn allows_valid_entity_workstream_transition() -> Result<()> {
        let pool = test_pool().await?;
        let executor = GenericCrudExecutor::new(pool.clone());

        let (case_id, entity_id): (Uuid, Uuid) = sqlx::query_as(
            r#"
            SELECT c.case_id, e.entity_id
            FROM "ob-poc".cases c
            CROSS JOIN "ob-poc".entities e
            LEFT JOIN "ob-poc".entity_workstreams w
              ON w.case_id = c.case_id
             AND w.entity_id = e.entity_id
            WHERE w.workstream_id IS NULL
            LIMIT 1
            "#,
        )
        .fetch_one(&pool)
        .await?;

        let workstream_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_workstreams (workstream_id, case_id, entity_id, status)
            VALUES ($1, $2, $3, 'PENDING')
            "#,
        )
        .bind(workstream_id)
        .bind(case_id)
        .bind(entity_id)
        .execute(&pool)
        .await?;

        let runtime_verb = runtime_registry()
            .get("entity-workstream", "update-status")
            .expect("entity-workstream.update-status must exist");

        let args = HashMap::from([
            (
                "workstream-id".to_string(),
                json!(workstream_id.to_string()),
            ),
            ("status".to_string(), json!("COLLECT")),
        ]);

        let result = executor.execute(runtime_verb, &args).await?;
        assert!(matches!(result, GenericExecutionResult::Affected(1)));

        let current_status: String = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".entity_workstreams WHERE workstream_id = $1"#,
        )
        .bind(workstream_id)
        .fetch_one(&pool)
        .await?;
        assert_eq!(current_status, "COLLECT");

        sqlx::query(r#"DELETE FROM "ob-poc".entity_workstreams WHERE workstream_id = $1"#)
            .bind(workstream_id)
            .execute(&pool)
            .await?;

        Ok(())
    }
}
