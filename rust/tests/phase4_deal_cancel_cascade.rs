//! Phase 4g gate (state-graph remediation): proves deal.cancel cascades to
//! non-terminal rate cards (deal_dag.yaml's deal_cancel_cascades rule was
//! declared but never implemented before this phase).

#[cfg(feature = "database")]
mod cascade {
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

        async fn create_contract(&self, client_label: &str) -> Uuid {
            sqlx::query_scalar(
                r#"INSERT INTO "ob-poc".legal_contracts (client_label, effective_date)
                   VALUES ($1, CURRENT_DATE) RETURNING contract_id"#,
            )
            .bind(client_label)
            .fetch_one(&self.pool)
            .await
            .unwrap()
        }

        async fn create_product(&self, name: &str) -> Uuid {
            sqlx::query_scalar(
                r#"INSERT INTO "ob-poc".products (name) VALUES ($1) RETURNING product_id"#,
            )
            .bind(name)
            .fetch_one(&self.pool)
            .await
            .unwrap()
        }

        async fn cleanup(&self) {
            let pattern = format!("{}%", self.prefix);
            sqlx::query(
                r#"DELETE FROM "ob-poc".deal_rate_cards WHERE deal_id IN
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
            sqlx::query(r#"DELETE FROM "ob-poc".legal_contracts WHERE client_label LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();
            sqlx::query(r#"DELETE FROM "ob-poc".products WHERE name LIKE $1"#)
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

    /// Cancel a deal with a PROPOSED rate card -> the card is cascaded to
    /// CANCELLED.
    #[tokio::test]
    async fn deal_cancel_cascades_proposed_rate_card_to_cancelled() -> Result<()> {
        let db = TestDb::new().await?;
        let group_id = db.create_client_group(&db.name("CascadeGroup")).await;

        let deal_ctx = db
            .execute_dsl(&format!(
                r#"(deal.create :deal-name "{}" :primary-client-group-id "{group_id}" :as @deal)"#,
                db.name("CascadeDeal")
            ))
            .await?;
        let deal_id = deal_ctx.resolve("deal").expect("deal id bound");
        let contract_id = db.create_contract(&db.name("Contract")).await;
        let product_id = db.create_product(&db.name("Product")).await;

        let rate_card_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".deal_rate_cards
                (deal_id, contract_id, product_id, rate_card_name, effective_from, status, negotiation_round)
               VALUES ($1, $2, $3, 'Test Card', CURRENT_DATE, 'PROPOSED', 1)
               RETURNING rate_card_id"#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .bind(product_id)
        .fetch_one(&db.pool)
        .await?;

        db.execute_dsl(&format!(r#"(deal.cancel :deal-id "{deal_id}" :reason "test cascade")"#))
            .await?;

        let deal_status: String =
            sqlx::query_scalar(r#"SELECT deal_status FROM "ob-poc".deals WHERE deal_id = $1"#)
                .bind(deal_id)
                .fetch_one(&db.pool)
                .await?;
        assert_eq!(deal_status, "CANCELLED");

        let card_status: String = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".deal_rate_cards WHERE rate_card_id = $1"#,
        )
        .bind(rate_card_id)
        .fetch_one(&db.pool)
        .await?;
        assert_eq!(
            card_status, "CANCELLED",
            "PROPOSED rate card must cascade to CANCELLED when its deal is cancelled"
        );

        db.cleanup().await;
        Ok(())
    }

    /// An already-AGREED rate card is terminal-ish and must NOT be
    /// cascaded (it is not in the non-terminal set).
    #[tokio::test]
    async fn deal_cancel_does_not_cascade_agreed_rate_card() -> Result<()> {
        let db = TestDb::new().await?;
        let group_id = db.create_client_group(&db.name("CascadeGroupAgreed")).await;

        let deal_ctx = db
            .execute_dsl(&format!(
                r#"(deal.create :deal-name "{}" :primary-client-group-id "{group_id}" :as @deal)"#,
                db.name("CascadeDealAgreed")
            ))
            .await?;
        let deal_id = deal_ctx.resolve("deal").expect("deal id bound");
        let contract_id = db.create_contract(&db.name("ContractAgreed")).await;
        let product_id = db.create_product(&db.name("ProductAgreed")).await;

        let rate_card_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".deal_rate_cards
                (deal_id, contract_id, product_id, rate_card_name, effective_from, status, negotiation_round)
               VALUES ($1, $2, $3, 'Agreed Card', CURRENT_DATE, 'AGREED', 1)
               RETURNING rate_card_id"#,
        )
        .bind(deal_id)
        .bind(contract_id)
        .bind(product_id)
        .fetch_one(&db.pool)
        .await?;

        db.execute_dsl(&format!(r#"(deal.cancel :deal-id "{deal_id}" :reason "test no-cascade")"#))
            .await?;

        let card_status: String = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".deal_rate_cards WHERE rate_card_id = $1"#,
        )
        .bind(rate_card_id)
        .fetch_one(&db.pool)
        .await?;
        assert_eq!(card_status, "AGREED", "AGREED cards must not be cascaded");

        db.cleanup().await;
        Ok(())
    }
}
