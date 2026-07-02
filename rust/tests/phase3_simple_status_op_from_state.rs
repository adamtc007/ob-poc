//! Phase 3c gate (state-graph remediation, RW-2): proves SimpleStatusOp's new
//! from-state enforcement actually rejects illegal transitions. Before this
//! phase, SimpleStatusOp blind-wrote the target status regardless of the
//! entity's current state.

#[cfg(feature = "database")]
mod red {
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

        async fn create_client_group(&self, name: &str) -> Uuid {
            sqlx::query_scalar(
                r#"INSERT INTO "ob-poc".client_group (canonical_name) VALUES ($1) RETURNING id"#,
            )
            .bind(name)
            .fetch_one(&self.pool)
            .await
            .unwrap()
        }

        async fn cleanup(&self) {
            let pattern = format!("{}%", self.prefix);
            sqlx::query(
                r#"DELETE FROM "ob-poc".deal_slas WHERE deal_id IN
                   (SELECT deal_id FROM "ob-poc".deals WHERE deal_name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();
            sqlx::query(r#"DELETE FROM "ob-poc".deals WHERE deal_name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();
            sqlx::query(r#"DELETE FROM "ob-poc".client_group WHERE canonical_name LIKE $1"#)
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

    /// RED: deal.mark-lost on a CONTRACTED deal is REJECTED (CONTRACTED is
    /// not in deal.mark-lost's requires_states: [PROSPECT, QUALIFYING,
    /// NEGOTIATING, IN_CLEARANCE]).
    #[tokio::test]
    async fn deal_mark_lost_on_contracted_deal_is_rejected() -> Result<()> {
        let db = TestDb::new().await?;
        let group_id = db.create_client_group(&db.name("Group")).await;

        let deal_ctx = db
            .execute_dsl(&format!(
                r#"(deal.create :deal-name "{}" :primary-client-group-id "{group_id}" :as @deal)"#,
                db.name("Deal")
            ))
            .await?;
        let deal_id = deal_ctx.resolve("deal").expect("deal id bound");

        sqlx::query(r#"UPDATE "ob-poc".deals SET deal_status = 'CONTRACTED' WHERE deal_id = $1"#)
            .bind(deal_id)
            .execute(&db.pool)
            .await?;

        let result = db
            .execute_dsl(&format!(
                r#"(deal.mark-lost :deal-id "{deal_id}" :reason "test")"#
            ))
            .await;
        assert!(
            result.is_err(),
            "expected deal.mark-lost on a CONTRACTED deal to be rejected, got Ok"
        );

        let status: String =
            sqlx::query_scalar(r#"SELECT deal_status FROM "ob-poc".deals WHERE deal_id = $1"#)
                .bind(deal_id)
                .fetch_one(&db.pool)
                .await?;
        assert_eq!(
            status, "CONTRACTED",
            "status must be unchanged after rejection"
        );

        db.cleanup().await;
        Ok(())
    }

    /// RED: deal.start-sla-remediation on a non-BREACHED SLA is REJECTED
    /// (requires_states: [BREACHED]).
    #[tokio::test]
    async fn deal_start_sla_remediation_on_non_breached_sla_is_rejected() -> Result<()> {
        let db = TestDb::new().await?;
        let group_id = db.create_client_group(&db.name("SlaGroup")).await;

        let deal_ctx = db
            .execute_dsl(&format!(
                r#"(deal.create :deal-name "{}" :primary-client-group-id "{group_id}" :as @deal)"#,
                db.name("SlaDeal")
            ))
            .await?;
        let deal_id = deal_ctx.resolve("deal").expect("deal id bound");

        let sla_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".deal_slas
                (deal_id, sla_name, metric_name, target_value, effective_from, sla_status)
               VALUES ($1, 'Test SLA', 'uptime', '99.9', CURRENT_DATE, 'ACTIVE')
               RETURNING sla_id"#,
        )
        .bind(deal_id)
        .fetch_one(&db.pool)
        .await?;

        let result = db
            .execute_dsl(&format!(
                r#"(deal.start-sla-remediation :sla-id "{sla_id}")"#
            ))
            .await;
        assert!(
            result.is_err(),
            "expected deal.start-sla-remediation on an ACTIVE (non-BREACHED) SLA to be rejected, got Ok"
        );

        let status: String =
            sqlx::query_scalar(r#"SELECT sla_status FROM "ob-poc".deal_slas WHERE sla_id = $1"#)
                .bind(sla_id)
                .fetch_one(&db.pool)
                .await?;
        assert_eq!(status, "ACTIVE", "status must be unchanged after rejection");

        db.cleanup().await;
        Ok(())
    }

    /// GREEN control: deal.start-sla-remediation on a BREACHED SLA succeeds.
    #[tokio::test]
    async fn deal_start_sla_remediation_on_breached_sla_succeeds() -> Result<()> {
        let db = TestDb::new().await?;
        let group_id = db.create_client_group(&db.name("SlaGroupOk")).await;

        let deal_ctx = db
            .execute_dsl(&format!(
                r#"(deal.create :deal-name "{}" :primary-client-group-id "{group_id}" :as @deal)"#,
                db.name("SlaDealOk")
            ))
            .await?;
        let deal_id = deal_ctx.resolve("deal").expect("deal id bound");

        let sla_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".deal_slas
                (deal_id, sla_name, metric_name, target_value, effective_from, sla_status)
               VALUES ($1, 'Test SLA', 'uptime', '99.9', CURRENT_DATE, 'BREACHED')
               RETURNING sla_id"#,
        )
        .bind(deal_id)
        .fetch_one(&db.pool)
        .await?;

        let result = db
            .execute_dsl(&format!(
                r#"(deal.start-sla-remediation :sla-id "{sla_id}")"#
            ))
            .await;
        assert!(result.is_ok(), "expected success: {:?}", result.err());

        let status: String =
            sqlx::query_scalar(r#"SELECT sla_status FROM "ob-poc".deal_slas WHERE sla_id = $1"#)
                .bind(sla_id)
                .fetch_one(&db.pool)
                .await?;
        assert_eq!(status, "IN_REMEDIATION");

        db.cleanup().await;
        Ok(())
    }
}
