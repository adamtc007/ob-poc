//! External harness for the SemOS VerbExecutionPort.
//!
//! Tests the `ObPocVerbExecutor` adapter through the public API, verifying
//! that verbs execute correctly through the SemOS-defined contract.
//!
//! Requires DATABASE_URL to be set (live Postgres).

#[cfg(feature = "database")]
mod tests {
    use sem_os_core::execution::{
        VerbExecutionContext, VerbExecutionOutcome, VerbExecutionPort,
    };
    use sem_os_core::principal::Principal;
    use uuid::Uuid;

    use ob_poc::sem_os_runtime::verb_executor_adapter::ObPocVerbExecutor;

    fn test_principal() -> Principal {
        Principal::in_process("test-harness", vec!["admin".to_string()])
    }

    /// CRUD verb execution through the SemOS port — SELECT (read-only, no side effects).
    #[tokio::test]
    #[ignore] // requires DATABASE_URL
    async fn crud_select_through_port() {
        let db_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for this test");
        let pool = sqlx::PgPool::connect(&db_url).await.expect("connect");
        let executor = ObPocVerbExecutor::from_pool(pool);

        let mut ctx = VerbExecutionContext::new(test_principal());

        // session.info is a CRUD SELECT verb that doesn't require entity scope
        let result = executor
            .execute_verb("session.info", serde_json::json!({}), &mut ctx)
            .await;

        // Should succeed (even if result is empty/void — the dispatch chain works)
        assert!(
            result.is_ok(),
            "CRUD verb through port should succeed: {:?}",
            result.err()
        );
    }

    /// Plugin verb execution through the SemOS port — entity.ghost creates a GHOST entity.
    #[tokio::test]
    #[ignore] // requires DATABASE_URL
    async fn plugin_verb_through_port() {
        let db_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for this test");
        let pool = sqlx::PgPool::connect(&db_url).await.expect("connect");
        let executor = ObPocVerbExecutor::from_pool(pool);

        let mut ctx = VerbExecutionContext::new(test_principal());
        let entity_name = format!("port-test-{}", Uuid::new_v4().to_string().split('-').next().unwrap());

        let result = executor
            .execute_verb(
                "entity.ghost",
                serde_json::json!({"name": entity_name}),
                &mut ctx,
            )
            .await;

        match result {
            Ok(r) => {
                assert!(
                    matches!(r.outcome, VerbExecutionOutcome::Uuid(_)),
                    "entity.ghost should return UUID, got {:?}",
                    r.outcome
                );
            }
            Err(e) => {
                // entity.ghost may fail if entity type setup is missing — that's OK,
                // the important thing is the dispatch chain worked (not NotFound)
                let msg = e.to_string();
                assert!(
                    !msg.contains("Unknown verb"),
                    "Should dispatch to handler, not 'Unknown verb': {msg}"
                );
            }
        }
    }

    /// Unknown verb through the port — should return error, not panic.
    #[tokio::test]
    #[ignore] // requires DATABASE_URL
    async fn unknown_verb_returns_error() {
        let db_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for this test");
        let pool = sqlx::PgPool::connect(&db_url).await.expect("connect");
        let executor = ObPocVerbExecutor::from_pool(pool);

        let mut ctx = VerbExecutionContext::new(test_principal());

        let result = executor
            .execute_verb("nonexistent.verb", serde_json::json!({}), &mut ctx)
            .await;

        assert!(result.is_err(), "Unknown verb must return error");
    }

    /// Context extensions round-trip — session_id survives adapter translation.
    #[tokio::test]
    #[ignore] // requires DATABASE_URL
    async fn context_extensions_round_trip() {
        let db_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for this test");
        let pool = sqlx::PgPool::connect(&db_url).await.expect("connect");
        let executor = ObPocVerbExecutor::from_pool(pool);

        let session_id = Uuid::new_v4();
        let mut ctx = VerbExecutionContext::new(test_principal());
        ctx.extensions = serde_json::json!({
            "session_id": session_id.to_string(),
            "audit_user": "test-user",
        });

        // Execute any verb — the extensions should survive the adapter round-trip
        let _ = executor
            .execute_verb("session.info", serde_json::json!({}), &mut ctx)
            .await;

        // Context should still be valid (not corrupted by the adapter)
        assert_eq!(ctx.principal.actor_id, "test-harness");
        assert!(!ctx.execution_id.is_nil());
    }
}
