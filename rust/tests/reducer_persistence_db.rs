#[cfg(feature = "database")]
mod tests {
    use anyhow::Result;
    use ob_poc::state_reducer::{handle_state_derive, load_builtin_state_machine};
    use sqlx::PgPool;
    use uuid::Uuid;

    async fn test_pool() -> Result<PgPool> {
        let database_url =
            std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".into());
        Ok(PgPool::connect(&database_url).await?)
    }

    #[tokio::test]
    async fn persists_reducer_state_for_derived_slot() -> Result<()> {
        let pool = test_pool().await?;
        let sm = load_builtin_state_machine("entity_kyc_lifecycle")?;

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

        sqlx::query(
            r#"DELETE FROM sem_reg.reducer_states WHERE entity_type = 'entity' AND entity_id = $1"#,
        )
        .bind(entity_id)
        .execute(&pool)
        .await?;

        let result =
            handle_state_derive(&pool, cbu_id, entity_id, "entity.primary", None, &sm).await?;

        let persisted: Option<(String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT current_state, phase
            FROM sem_reg.reducer_states
            WHERE entity_type = 'entity' AND entity_id = $1
            "#,
        )
        .bind(entity_id)
        .fetch_optional(&pool)
        .await?;

        let (current_state, phase) = persisted.expect("reducer state should be persisted");
        assert_eq!(current_state, result.effective_state);
        assert_eq!(phase.as_deref(), Some(result.computed_state.as_str()));

        Ok(())
    }
}
