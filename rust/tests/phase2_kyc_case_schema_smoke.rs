//! Phase 2 gate (state-graph remediation, RW-1): proves kyc-case.assign and
//! case-event.log now execute against the live DB. Both previously targeted
//! `crud.schema: kyc` / `lookup.schema: kyc` -- a Postgres schema that does
//! not exist (the live table is `"ob-poc".cases` / `"ob-poc".case_events`) --
//! so every invocation errored before the schema-name fix.

#[cfg(feature = "database")]
mod smoke {
    use anyhow::Result;
    use sqlx::PgPool;
    use std::path::PathBuf;
    use uuid::Uuid;

    use ob_poc::dsl_v2::execution::{DslExecutor, ExecutionContext};
    use ob_poc::dsl_v2::planning::compile;
    use ob_poc::dsl_v2::syntax::parse_program;

    struct TestDb {
        pool: PgPool,
        prefix: String,
    }

    impl TestDb {
        async fn new() -> Result<Self> {
            let config_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config");
            std::env::set_var("DSL_CONFIG_DIR", &config_dir);

            let url = std::env::var("TEST_DATABASE_URL")
                .or_else(|_| std::env::var("DATABASE_URL"))
                .unwrap_or_else(|_| "postgresql:///data_designer".into());

            let pool = PgPool::connect(&url).await?;
            let prefix = format!("test_{}", &Uuid::new_v4().to_string()[..8]);
            Ok(Self { pool, prefix })
        }

        fn name(&self, base: &str) -> String {
            format!("{}_{}", self.prefix, base)
        }

        async fn execute_dsl(&self, dsl: &str) -> Result<ExecutionContext> {
            let ast = parse_program(dsl).map_err(|e| anyhow::anyhow!("{:?}", e))?;
            let plan = compile(&ast).map_err(|e| anyhow::anyhow!("{:?}", e))?;
            let executor = DslExecutor::new(self.pool.clone()).with_sem_os_ops(sem_os_registry());
            let mut ctx = ExecutionContext::new();
            executor.execute_plan(&plan, &mut ctx).await?;
            Ok(ctx)
        }

        async fn cleanup(&self) {
            let pattern = format!("{}%", self.prefix);
            sqlx::query(
                r#"DELETE FROM "ob-poc".case_events WHERE case_id IN
                   (SELECT c.case_id FROM "ob-poc".cases c
                    JOIN "ob-poc".cbus cbu ON cbu.cbu_id = c.cbu_id
                    WHERE cbu.name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();
            sqlx::query(
                r#"DELETE FROM "ob-poc".cases WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();
            sqlx::query(
                r#"DELETE FROM "ob-poc".entities WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();
            sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();
        }
    }

    fn sem_os_registry() -> std::sync::Arc<sem_os_postgres::ops::SemOsVerbOpRegistry> {
        use std::sync::OnceLock;
        static REGISTRY: OnceLock<std::sync::Arc<sem_os_postgres::ops::SemOsVerbOpRegistry>> =
            OnceLock::new();
        REGISTRY
            .get_or_init(|| {
                let mut reg = sem_os_postgres::ops::build_registry();
                ob_poc::domain_ops::extend_registry(&mut reg);
                std::sync::Arc::new(reg)
            })
            .clone()
    }

    #[tokio::test]
    async fn test_kyc_case_assign_and_case_event_log_succeed() -> Result<()> {
        let db = TestDb::new().await?;

        let cbu_ctx = db
            .execute_dsl(&format!(
                r#"(cbu.create :name "{}" :as @cbu)"#,
                db.name("Phase2SmokeCBU")
            ))
            .await?;
        let cbu_id = cbu_ctx.resolve("cbu").expect("cbu id bound");

        let analyst_ctx = db
            .execute_dsl(&format!(
                r#"(entity.create :entity-type "proper-person" :cbu-id "{cbu_id}" :first-name "Ana" :last-name "Lyst" :as @analyst)"#
            ))
            .await?;
        let analyst_id = analyst_ctx.resolve("analyst").expect("analyst id bound");

        let case_ctx = db
            .execute_dsl(&format!(
                r#"(kyc-case.create :cbu-id "{cbu_id}" :as @case)"#
            ))
            .await?;
        let case_id = case_ctx.resolve("case").expect("case id bound");

        // Target verb #1: kyc-case.assign -- previously errored, crud.schema: kyc
        // does not exist as a Postgres schema.
        let assign_result = db
            .execute_dsl(&format!(
                r#"(kyc-case.assign :case-id "{case_id}" :analyst-id "{analyst_id}")"#
            ))
            .await;
        assert!(
            assign_result.is_ok(),
            "kyc-case.assign failed: {:?}",
            assign_result.err()
        );

        // Target verb #2: case-event.log -- previously errored, same schema bug
        // on both crud.schema and the case-id lookup.schema.
        let log_result = db
            .execute_dsl(&format!(
                r#"(case-event.log :case-id "{case_id}" :event-type "PHASE2_SMOKE_TEST")"#
            ))
            .await;
        assert!(
            log_result.is_ok(),
            "case-event.log failed: {:?}",
            log_result.err()
        );

        let assigned: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT assigned_analyst_id FROM "ob-poc".cases WHERE case_id = $1"#,
        )
        .bind(case_id)
        .fetch_one(&db.pool)
        .await?;
        assert_eq!(assigned, Some(analyst_id));

        let event_count: i64 = sqlx::query_scalar(
            r#"SELECT count(*) FROM "ob-poc".case_events WHERE case_id = $1 AND event_type = 'PHASE2_SMOKE_TEST'"#,
        )
        .bind(case_id)
        .fetch_one(&db.pool)
        .await?;
        assert_eq!(event_count, 1);

        db.cleanup().await;
        Ok(())
    }
}
