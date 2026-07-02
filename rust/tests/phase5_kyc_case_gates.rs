//! Phase 5 gate (state-graph remediation): proves the case-decision gates
//! that were declared in kyc_dag.yaml but never enforced.

#[cfg(feature = "database")]
mod gates {
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

        /// Create a CBU + a REVIEW-status case with one workstream in the
        /// given status, via raw SQL (kyc-case.create always lands INTAKE;
        /// no verb path advances a case straight to REVIEW).
        async fn create_review_case_with_workstream(
            &self,
            cbu_name: &str,
            workstream_status: &str,
        ) -> (Uuid, Uuid) {
            let cbu_id: Uuid = sqlx::query_scalar(
                r#"INSERT INTO "ob-poc".cbus (name) VALUES ($1) RETURNING cbu_id"#,
            )
            .bind(cbu_name)
            .fetch_one(&self.pool)
            .await
            .unwrap();
            let case_id: Uuid = sqlx::query_scalar(
                r#"INSERT INTO "ob-poc".cases (cbu_id, status, case_ref)
                   VALUES ($1, 'REVIEW', $2) RETURNING case_id"#,
            )
            .bind(cbu_id)
            .bind(format!("KYC-TEST-{}", &Uuid::new_v4().to_string()[..8]))
            .fetch_one(&self.pool)
            .await
            .unwrap();
            let entity_id: Uuid =
                sqlx::query_scalar(r#"SELECT entity_id FROM "ob-poc".entities LIMIT 1"#)
                    .fetch_optional(&self.pool)
                    .await
                    .unwrap()
                    .unwrap_or_else(|| {
                        panic!("fixture requires at least one existing entities row in the test DB")
                    });
            sqlx::query(
                r#"INSERT INTO "ob-poc".entity_workstreams (case_id, entity_id, status, is_ubo)
                   VALUES ($1, $2, $3, false)"#,
            )
            .bind(case_id)
            .bind(entity_id)
            .bind(workstream_status)
            .execute(&self.pool)
            .await
            .unwrap();
            (cbu_id, case_id)
        }

        async fn cleanup_case(&self, cbu_id: Uuid) {
            sqlx::query(
                r#"DELETE FROM "ob-poc".entity_workstreams WHERE case_id IN
                   (SELECT case_id FROM "ob-poc".cases WHERE cbu_id = $1)"#,
            )
            .bind(cbu_id)
            .execute(&self.pool)
            .await
            .ok();
            sqlx::query(r#"DELETE FROM "ob-poc".cases WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .execute(&self.pool)
                .await
                .ok();
            sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE cbu_id = $1"#)
                .bind(cbu_id)
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

    /// RED (Phase 5c): kyc-case.approve with an ASSESSMENT-state workstream
    /// is rejected.
    #[tokio::test]
    async fn approve_rejected_when_workstream_incomplete() -> Result<()> {
        let db = TestDb::new().await?;
        let (cbu_id, case_id) = db
            .create_review_case_with_workstream(&db.name("Phase5ApproveCBU"), "ASSESS")
            .await;

        let result = db
            .execute_dsl(&format!(r#"(kyc-case.approve :case-id "{case_id}")"#))
            .await;
        assert!(
            result.is_err(),
            "expected approve to be rejected with an incomplete workstream"
        );

        let status: String =
            sqlx::query_scalar(r#"SELECT status FROM "ob-poc".cases WHERE case_id = $1"#)
                .bind(case_id)
                .fetch_one(&db.pool)
                .await?;
        assert_eq!(
            status, "REVIEW",
            "case must remain in REVIEW after rejection"
        );

        db.cleanup_case(cbu_id).await;
        Ok(())
    }

    /// GREEN control: approve succeeds once the workstream is COMPLETE.
    #[tokio::test]
    async fn approve_succeeds_when_workstream_complete() -> Result<()> {
        let db = TestDb::new().await?;
        let (cbu_id, case_id) = db
            .create_review_case_with_workstream(&db.name("Phase5ApproveOkCBU"), "COMPLETE")
            .await;

        let result = db
            .execute_dsl(&format!(r#"(kyc-case.approve :case-id "{case_id}")"#))
            .await;
        assert!(
            result.is_ok(),
            "expected approve to succeed: {:?}",
            result.err()
        );

        let status: String =
            sqlx::query_scalar(r#"SELECT status FROM "ob-poc".cases WHERE case_id = $1"#)
                .bind(case_id)
                .fetch_one(&db.pool)
                .await?;
        assert_eq!(status, "APPROVED");

        db.cleanup_case(cbu_id).await;
        Ok(())
    }

    /// RED (Phase 5d): kyc-case.escalate at BOARD level sets
    /// cases.status = REFER_TO_REGULATOR.
    #[tokio::test]
    async fn escalate_to_board_sets_refer_to_regulator() -> Result<()> {
        let db = TestDb::new().await?;
        let (cbu_id, case_id) = db
            .create_review_case_with_workstream(&db.name("Phase5EscalateCBU"), "ASSESS")
            .await;

        db.execute_dsl(&format!(
            r#"(kyc-case.escalate :case-id "{case_id}" :escalation-level "BOARD")"#
        ))
        .await?;

        let (status, level): (String, String) = sqlx::query_as(
            r#"SELECT status, escalation_level FROM "ob-poc".cases WHERE case_id = $1"#,
        )
        .bind(case_id)
        .fetch_one(&db.pool)
        .await?;
        assert_eq!(status, "REFER_TO_REGULATOR");
        assert_eq!(level, "BOARD");

        db.cleanup_case(cbu_id).await;
        Ok(())
    }

    /// GREEN control: a STANDARD-level escalation does NOT touch case status.
    #[tokio::test]
    async fn escalate_standard_does_not_refer_to_regulator() -> Result<()> {
        let db = TestDb::new().await?;
        let (cbu_id, case_id) = db
            .create_review_case_with_workstream(&db.name("Phase5EscalateStdCBU"), "ASSESS")
            .await;

        db.execute_dsl(&format!(
            r#"(kyc-case.escalate :case-id "{case_id}" :escalation-level "STANDARD")"#
        ))
        .await?;

        let status: String =
            sqlx::query_scalar(r#"SELECT status FROM "ob-poc".cases WHERE case_id = $1"#)
                .bind(case_id)
                .fetch_one(&db.pool)
                .await?;
        assert_eq!(status, "REVIEW");

        db.cleanup_case(cbu_id).await;
        Ok(())
    }

    /// RED (Phase 5e): kyc-case.refer sets REFER_TO_REGULATOR and cascades
    /// the non-terminal workstream to REFERRED.
    #[tokio::test]
    async fn refer_cascades_workstreams_to_referred() -> Result<()> {
        let db = TestDb::new().await?;
        let (cbu_id, case_id) = db
            .create_review_case_with_workstream(&db.name("Phase5ReferCBU"), "ASSESS")
            .await;

        db.execute_dsl(&format!(
            r#"(kyc-case.refer :case-id "{case_id}" :reason "regulatory referral")"#
        ))
        .await?;

        let case_status: String =
            sqlx::query_scalar(r#"SELECT status FROM "ob-poc".cases WHERE case_id = $1"#)
                .bind(case_id)
                .fetch_one(&db.pool)
                .await?;
        assert_eq!(case_status, "REFER_TO_REGULATOR");

        let ws_status: String = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".entity_workstreams WHERE case_id = $1"#,
        )
        .bind(case_id)
        .fetch_one(&db.pool)
        .await?;
        assert_eq!(ws_status, "REFERRED");

        db.cleanup_case(cbu_id).await;
        Ok(())
    }

    /// RED (Phase 5e): kyc-case.reject cascades non-terminal workstreams to
    /// PROHIBITED.
    #[tokio::test]
    async fn reject_cascades_workstreams_to_prohibited() -> Result<()> {
        let db = TestDb::new().await?;
        let (cbu_id, case_id) = db
            .create_review_case_with_workstream(&db.name("Phase5RejectCBU"), "ASSESS")
            .await;

        db.execute_dsl(&format!(
            r#"(kyc-case.reject :case-id "{case_id}" :reason "adverse findings")"#
        ))
        .await?;

        let case_status: String =
            sqlx::query_scalar(r#"SELECT status FROM "ob-poc".cases WHERE case_id = $1"#)
                .bind(case_id)
                .fetch_one(&db.pool)
                .await?;
        assert_eq!(case_status, "REJECTED");

        let ws_status: String = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".entity_workstreams WHERE case_id = $1"#,
        )
        .bind(case_id)
        .fetch_one(&db.pool)
        .await?;
        assert_eq!(ws_status, "PROHIBITED");

        db.cleanup_case(cbu_id).await;
        Ok(())
    }
}
