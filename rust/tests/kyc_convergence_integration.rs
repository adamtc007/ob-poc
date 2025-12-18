//! KYC Convergence Model Integration Tests
//!
//! These tests verify the complete convergence flow:
//! Allegations → Proofs → Observations → Verification → Convergence → Decision
//!
//! The convergence model ensures that all ownership/control claims are backed by
//! documentary evidence before a KYC decision can be made.

#[cfg(feature = "database")]
mod convergence_tests {
    use anyhow::Result;
    use sqlx::PgPool;
    use uuid::Uuid;

    use ob_poc::dsl_v2::{compile, parse_program, DslExecutor, ExecutionContext};

    // =========================================================================
    // TEST INFRASTRUCTURE
    // =========================================================================

    struct TestDb {
        pool: PgPool,
        prefix: String,
    }

    impl TestDb {
        async fn new() -> Result<Self> {
            let url = std::env::var("TEST_DATABASE_URL")
                .or_else(|_| std::env::var("DATABASE_URL"))
                .unwrap_or_else(|_| "postgresql:///data_designer".into());

            let pool = PgPool::connect(&url).await?;
            let prefix = format!("conv_{}", &Uuid::new_v4().to_string()[..8]);
            Ok(Self { pool, prefix })
        }

        fn name(&self, base: &str) -> String {
            format!("{}_{}", self.prefix, base)
        }

        async fn cleanup(&self) -> Result<()> {
            let pattern = format!("{}%", self.prefix);

            // Delete convergence model tables first
            sqlx::query(
                r#"DELETE FROM "ob-poc".ubo_assertion_log WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".kyc_decisions WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".ubo_observations WHERE edge_id IN
                   (SELECT edge_id FROM "ob-poc".ubo_edges e
                    JOIN "ob-poc".cbus c ON c.cbu_id = e.cbu_id
                    WHERE c.name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".proofs WHERE edge_id IN
                   (SELECT edge_id FROM "ob-poc".ubo_edges e
                    JOIN "ob-poc".cbus c ON c.cbu_id = e.cbu_id
                    WHERE c.name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".ubo_edges WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Delete kyc schema tables
            sqlx::query(
                r#"DELETE FROM kyc.screenings WHERE workstream_id IN
                   (SELECT w.workstream_id FROM kyc.entity_workstreams w
                    JOIN kyc.cases c ON c.case_id = w.case_id
                    JOIN "ob-poc".cbus cbu ON cbu.cbu_id = c.cbu_id
                    WHERE cbu.name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM kyc.entity_workstreams WHERE case_id IN
                   (SELECT c.case_id FROM kyc.cases c
                    JOIN "ob-poc".cbus cbu ON cbu.cbu_id = c.cbu_id
                    WHERE cbu.name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM kyc.cases WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Delete standard ob-poc tables
            sqlx::query(
                r#"DELETE FROM "ob-poc".document_catalog WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM "ob-poc".cbu_entity_roles WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(r#"DELETE FROM "ob-poc".entities WHERE name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();

            sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();

            Ok(())
        }

        async fn execute_dsl(&self, dsl: &str) -> Result<ExecutionContext> {
            let ast = parse_program(dsl).map_err(|e| anyhow::anyhow!("{:?}", e))?;
            let plan = compile(&ast).map_err(|e| anyhow::anyhow!("{:?}", e))?;
            let executor = DslExecutor::new(self.pool.clone());
            let mut ctx = ExecutionContext::new();
            executor.execute_plan(&plan, &mut ctx).await?;
            Ok(ctx)
        }
    }

    // =========================================================================
    // HELPER FUNCTIONS
    // =========================================================================

    /// Count edges in a specific state for a CBU
    async fn count_edges_by_state(pool: &PgPool, cbu_id: Uuid, state: &str) -> Result<i64> {
        let count: Option<i64> = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".ubo_edges WHERE cbu_id = $1 AND state = $2"#,
        )
        .bind(cbu_id)
        .bind(state)
        .fetch_one(pool)
        .await?;
        Ok(count.unwrap_or(0))
    }

    /// Count total edges for a CBU
    async fn count_total_edges(pool: &PgPool, cbu_id: Uuid) -> Result<i64> {
        let count: Option<i64> =
            sqlx::query_scalar(r#"SELECT COUNT(*) FROM "ob-poc".ubo_edges WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .fetch_one(pool)
                .await?;
        Ok(count.unwrap_or(0))
    }

    /// Count proofs linked to edges for a CBU
    async fn count_proofs(pool: &PgPool, cbu_id: Uuid) -> Result<i64> {
        let count: Option<i64> = sqlx::query_scalar(
            r#"SELECT COUNT(*) FROM "ob-poc".proofs p
               JOIN "ob-poc".ubo_edges e ON e.edge_id = p.edge_id
               WHERE e.cbu_id = $1"#,
        )
        .bind(cbu_id)
        .fetch_one(pool)
        .await?;
        Ok(count.unwrap_or(0))
    }

    /// Check if graph is converged for a CBU
    async fn is_converged(pool: &PgPool, cbu_id: Uuid) -> Result<bool> {
        let converged: Option<bool> = sqlx::query_scalar(
            r#"SELECT is_converged FROM "ob-poc".ubo_convergence_status WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .fetch_optional(pool)
        .await?;
        Ok(converged.unwrap_or(false))
    }

    /// Get convergence percentage for a CBU
    async fn get_convergence_percentage(pool: &PgPool, cbu_id: Uuid) -> Result<f64> {
        let row: Option<(i64, i64)> = sqlx::query_as(
            r#"SELECT total_edges, proven_edges FROM "ob-poc".ubo_convergence_status WHERE cbu_id = $1"#,
        )
        .bind(cbu_id)
        .fetch_optional(pool)
        .await?;

        match row {
            Some((total, proven)) if total > 0 => Ok((proven as f64 / total as f64) * 100.0),
            _ => Ok(0.0),
        }
    }

    /// Count KYC decisions for a CBU
    async fn count_decisions(pool: &PgPool, cbu_id: Uuid) -> Result<i64> {
        let count: Option<i64> =
            sqlx::query_scalar(r#"SELECT COUNT(*) FROM "ob-poc".kyc_decisions WHERE cbu_id = $1"#)
                .bind(cbu_id)
                .fetch_one(pool)
                .await?;
        Ok(count.unwrap_or(0))
    }

    /// Get latest decision for a CBU
    async fn get_latest_decision(pool: &PgPool, cbu_id: Uuid) -> Result<Option<String>> {
        let decision: Option<String> = sqlx::query_scalar(
            r#"SELECT decision FROM "ob-poc".kyc_decisions
               WHERE cbu_id = $1 ORDER BY decided_at DESC LIMIT 1"#,
        )
        .bind(cbu_id)
        .fetch_optional(pool)
        .await?;
        Ok(decision)
    }

    // =========================================================================
    // PHASE 1: ALLEGATION TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_allege_ownership_creates_edge() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(
            r#"
            (cbu.create :name "{}" :client-type "corporate" :jurisdiction "GB" :as @cbu)
            (entity.create-limited-company :name "{}" :jurisdiction "GB" :as @company)
            (entity.create-proper-person :first-name "John" :last-name "Smith" :as @ubo)
            (ubo.allege :cbu-id @cbu :from-entity-id @ubo :to-entity-id @company
                        :edge-type "ownership" :percentage 100 :as @edge)
        "#,
            db.name("AllegeCBU"),
            db.name("AllegeCompany")
        );

        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").expect("cbu should be bound");

        // Verify edge was created in alleged state
        let alleged_count = count_edges_by_state(&db.pool, cbu_id, "alleged").await?;
        assert_eq!(alleged_count, 1, "Should have 1 alleged edge");

        let total = count_total_edges(&db.pool, cbu_id).await?;
        assert_eq!(total, 1, "Should have 1 total edge");

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_allege_control_creates_edge() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(
            r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :name "{}" :as @company)
            (entity.create-proper-person :first-name "Jane" :last-name "Director" :as @director)
            (ubo.allege :cbu-id @cbu :from-entity-id @director :to-entity-id @company
                        :edge-type "control" :as @edge)
        "#,
            db.name("ControlCBU"),
            db.name("ControlCompany")
        );

        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").expect("cbu should be bound");

        let alleged_count = count_edges_by_state(&db.pool, cbu_id, "alleged").await?;
        assert_eq!(alleged_count, 1, "Should have 1 alleged control edge");

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_multiple_allegations_build_graph() -> Result<()> {
        let db = TestDb::new().await?;

        // Build ownership chain: UBO -> HoldCo -> OpCo
        let dsl = format!(
            r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :name "{}" :as @opco)
            (entity.create-limited-company :name "{}" :as @holdco)
            (entity.create-proper-person :first-name "Ultimate" :last-name "Owner" :as @ubo)

            (ubo.allege :cbu-id @cbu :from-entity-id @holdco :to-entity-id @opco
                        :edge-type "ownership" :percentage 100)
            (ubo.allege :cbu-id @cbu :from-entity-id @ubo :to-entity-id @holdco
                        :edge-type "ownership" :percentage 100)
        "#,
            db.name("ChainCBU"),
            db.name("OpCo"),
            db.name("HoldCo")
        );

        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").expect("cbu should be bound");

        let total = count_total_edges(&db.pool, cbu_id).await?;
        assert_eq!(total, 2, "Should have 2 edges in ownership chain");

        // Graph should not be converged yet (no proofs)
        let converged = is_converged(&db.pool, cbu_id).await?;
        assert!(!converged, "Graph should not be converged without proofs");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // PHASE 2: PROOF LINKING TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_link_proof_transitions_edge_to_pending() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(
            r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :name "{}" :as @company)
            (entity.create-proper-person :first-name "Proven" :last-name "Owner" :as @owner)

            (ubo.allege :cbu-id @cbu :from-entity-id @owner :to-entity-id @company
                        :edge-type "ownership" :percentage 75 :as @edge)

            (document.catalog :cbu-id @cbu :doc-type "SHARE_CERTIFICATE"
                              :title "Share Certificate" :as @doc)

            (ubo.link-proof :edge-id @edge :document-id @doc)
        "#,
            db.name("ProofCBU"),
            db.name("ProofCompany")
        );

        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").expect("cbu should be bound");

        // Edge should now be in pending state (has proof but not verified)
        let pending_count = count_edges_by_state(&db.pool, cbu_id, "pending").await?;
        assert_eq!(pending_count, 1, "Should have 1 pending edge after linking proof");

        let proof_count = count_proofs(&db.pool, cbu_id).await?;
        assert_eq!(proof_count, 1, "Should have 1 proof linked");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // PHASE 3: VERIFICATION TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_verify_transitions_edge_to_proven() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(
            r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :name "{}" :as @company)
            (entity.create-proper-person :first-name "Verified" :last-name "Owner" :as @owner)

            (ubo.allege :cbu-id @cbu :from-entity-id @owner :to-entity-id @company
                        :edge-type "ownership" :percentage 50 :as @edge)

            (document.catalog :cbu-id @cbu :doc-type "REGISTER_OF_SHAREHOLDERS"
                              :title "Shareholder Register" :as @doc)

            (ubo.link-proof :edge-id @edge :document-id @doc)
            (ubo.verify :edge-id @edge)
        "#,
            db.name("VerifyCBU"),
            db.name("VerifyCompany")
        );

        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").expect("cbu should be bound");

        // Edge should now be proven
        let proven_count = count_edges_by_state(&db.pool, cbu_id, "proven").await?;
        assert_eq!(proven_count, 1, "Should have 1 proven edge after verification");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // PHASE 4: CONVERGENCE TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_full_convergence_flow() -> Result<()> {
        let db = TestDb::new().await?;

        // Complete flow: allege -> proof -> verify for each edge
        let dsl = format!(
            r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :name "{}" :as @opco)
            (entity.create-limited-company :name "{}" :as @holdco)
            (entity.create-proper-person :first-name "Ultimate" :last-name "Beneficial" :as @ubo)

            ;; Edge 1: HoldCo owns OpCo
            (ubo.allege :cbu-id @cbu :from-entity-id @holdco :to-entity-id @opco
                        :edge-type "ownership" :percentage 100 :as @edge1)
            (document.catalog :cbu-id @cbu :doc-type "SHARE_CERTIFICATE"
                              :title "OpCo Shares" :as @doc1)
            (ubo.link-proof :edge-id @edge1 :document-id @doc1)
            (ubo.verify :edge-id @edge1)

            ;; Edge 2: UBO owns HoldCo
            (ubo.allege :cbu-id @cbu :from-entity-id @ubo :to-entity-id @holdco
                        :edge-type "ownership" :percentage 100 :as @edge2)
            (document.catalog :cbu-id @cbu :doc-type "REGISTER_OF_SHAREHOLDERS"
                              :title "HoldCo Register" :as @doc2)
            (ubo.link-proof :edge-id @edge2 :document-id @doc2)
            (ubo.verify :edge-id @edge2)
        "#,
            db.name("ConvergeCBU"),
            db.name("ConvergeOpCo"),
            db.name("ConvergeHoldCo")
        );

        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").expect("cbu should be bound");

        // All edges should be proven
        let proven = count_edges_by_state(&db.pool, cbu_id, "proven").await?;
        assert_eq!(proven, 2, "Should have 2 proven edges");

        // Graph should now be converged
        let converged = is_converged(&db.pool, cbu_id).await?;
        assert!(converged, "Graph should be converged when all edges are proven");

        let percentage = get_convergence_percentage(&db.pool, cbu_id).await?;
        assert!((percentage - 100.0).abs() < 0.01, "Convergence should be 100%");

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_partial_convergence() -> Result<()> {
        let db = TestDb::new().await?;

        // Only verify one of two edges
        let dsl = format!(
            r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :name "{}" :as @company)
            (entity.create-proper-person :first-name "Owner" :last-name "One" :as @owner1)
            (entity.create-proper-person :first-name "Owner" :last-name "Two" :as @owner2)

            ;; Edge 1: Fully verified
            (ubo.allege :cbu-id @cbu :from-entity-id @owner1 :to-entity-id @company
                        :edge-type "ownership" :percentage 50 :as @edge1)
            (document.catalog :cbu-id @cbu :doc-type "SHARE_CERTIFICATE" :title "Cert 1" :as @doc1)
            (ubo.link-proof :edge-id @edge1 :document-id @doc1)
            (ubo.verify :edge-id @edge1)

            ;; Edge 2: Only alleged (no proof)
            (ubo.allege :cbu-id @cbu :from-entity-id @owner2 :to-entity-id @company
                        :edge-type "ownership" :percentage 50)
        "#,
            db.name("PartialCBU"),
            db.name("PartialCompany")
        );

        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").expect("cbu should be bound");

        // Should not be converged
        let converged = is_converged(&db.pool, cbu_id).await?;
        assert!(!converged, "Graph should not be converged with unproven edges");

        let percentage = get_convergence_percentage(&db.pool, cbu_id).await?;
        assert!((percentage - 50.0).abs() < 0.01, "Convergence should be 50%");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // PHASE 5: STATUS CHECK TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_status_returns_convergence_info() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(
            r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :name "{}" :as @company)
            (entity.create-proper-person :first-name "Status" :last-name "Test" :as @owner)

            (ubo.allege :cbu-id @cbu :from-entity-id @owner :to-entity-id @company
                        :edge-type "ownership" :percentage 100 :as @edge)

            (ubo.status :cbu-id @cbu :as @status)
        "#,
            db.name("StatusCBU"),
            db.name("StatusCompany")
        );

        let ctx = db.execute_dsl(&dsl).await?;

        // Status should be bound
        let status_id = ctx.resolve("status");
        assert!(status_id.is_some(), "Status should be bound");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // PHASE 6: ASSERTION TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_assert_ownership_complete_passes() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(
            r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :name "{}" :as @company)
            (entity.create-proper-person :first-name "Complete" :last-name "Owner" :as @owner)

            (ubo.allege :cbu-id @cbu :from-entity-id @owner :to-entity-id @company
                        :edge-type "ownership" :percentage 100 :as @edge)
            (document.catalog :cbu-id @cbu :doc-type "SHARE_CERTIFICATE" :title "Cert" :as @doc)
            (ubo.link-proof :edge-id @edge :document-id @doc)
            (ubo.verify :edge-id @edge)

            (ubo.assert :cbu-id @cbu :assertion "ownership_complete")
        "#,
            db.name("AssertCBU"),
            db.name("AssertCompany")
        );

        // Should not error
        let result = db.execute_dsl(&dsl).await;
        assert!(result.is_ok(), "Assertion should pass for converged graph");

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // PHASE 7: DECISION TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_decision_after_convergence() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(
            r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :name "{}" :as @company)
            (entity.create-proper-person :first-name "Approved" :last-name "Owner" :as @owner)

            ;; Build converged graph
            (ubo.allege :cbu-id @cbu :from-entity-id @owner :to-entity-id @company
                        :edge-type "ownership" :percentage 100 :as @edge)
            (document.catalog :cbu-id @cbu :doc-type "SHARE_CERTIFICATE" :title "Cert" :as @doc)
            (ubo.link-proof :edge-id @edge :document-id @doc)
            (ubo.verify :edge-id @edge)

            ;; Record decision
            (kyc.decision :cbu-id @cbu :decision "APPROVED" :rationale "All proofs verified")
        "#,
            db.name("DecisionCBU"),
            db.name("DecisionCompany")
        );

        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").expect("cbu should be bound");

        let decision_count = count_decisions(&db.pool, cbu_id).await?;
        assert_eq!(decision_count, 1, "Should have 1 decision");

        let decision = get_latest_decision(&db.pool, cbu_id).await?;
        assert_eq!(decision, Some("APPROVED".to_string()));

        db.cleanup().await?;
        Ok(())
    }

    // =========================================================================
    // END-TO-END SCENARIO TESTS
    // =========================================================================

    #[tokio::test]
    async fn test_complete_kyc_convergence_scenario() -> Result<()> {
        let db = TestDb::new().await?;

        // Complete KYC scenario with multiple entities and ownership layers
        let dsl = format!(
            r#"
            ;; 1. Create CBU and structure
            (cbu.create :name "{}" :client-type "fund" :jurisdiction "LU" :as @cbu)

            ;; 2. Create entities
            (entity.create-limited-company :name "{}" :jurisdiction "LU" :as @fund)
            (entity.create-limited-company :name "{}" :jurisdiction "LU" :as @manco)
            (entity.create-proper-person :first-name "Director" :last-name "One" :as @director)
            (entity.create-proper-person :first-name "UBO" :last-name "Person" :as @ubo)

            ;; 3. Build allegation graph
            (ubo.allege :cbu-id @cbu :from-entity-id @manco :to-entity-id @fund
                        :edge-type "ownership" :percentage 100 :as @e1)
            (ubo.allege :cbu-id @cbu :from-entity-id @ubo :to-entity-id @manco
                        :edge-type "ownership" :percentage 60 :as @e2)
            (ubo.allege :cbu-id @cbu :from-entity-id @director :to-entity-id @manco
                        :edge-type "control" :as @e3)

            ;; 4. Collect proofs
            (document.catalog :cbu-id @cbu :doc-type "CERTIFICATE_OF_INCORPORATION"
                              :title "Fund Inc Cert" :as @d1)
            (document.catalog :cbu-id @cbu :doc-type "REGISTER_OF_SHAREHOLDERS"
                              :title "ManCo Register" :as @d2)
            (document.catalog :cbu-id @cbu :doc-type "BOARD_RESOLUTION"
                              :title "Director Appointment" :as @d3)

            ;; 5. Link proofs to edges
            (ubo.link-proof :edge-id @e1 :document-id @d1)
            (ubo.link-proof :edge-id @e2 :document-id @d2)
            (ubo.link-proof :edge-id @e3 :document-id @d3)

            ;; 6. Verify each edge
            (ubo.verify :edge-id @e1)
            (ubo.verify :edge-id @e2)
            (ubo.verify :edge-id @e3)

            ;; 7. Check convergence status
            (ubo.status :cbu-id @cbu :as @status)

            ;; 8. Run assertions
            (ubo.assert :cbu-id @cbu :assertion "ownership_complete")

            ;; 9. Evaluate and decide
            (ubo.evaluate :cbu-id @cbu :as @eval)
            (kyc.decision :cbu-id @cbu :decision "APPROVED" :rationale "Full convergence achieved")
        "#,
            db.name("E2E_Fund"),
            db.name("E2E_FundSICAV"),
            db.name("E2E_ManCo")
        );

        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").expect("cbu should be bound");

        // Verify final state
        let converged = is_converged(&db.pool, cbu_id).await?;
        assert!(converged, "Graph should be fully converged");

        let proven = count_edges_by_state(&db.pool, cbu_id, "proven").await?;
        assert_eq!(proven, 3, "All 3 edges should be proven");

        let decision = get_latest_decision(&db.pool, cbu_id).await?;
        assert_eq!(decision, Some("APPROVED".to_string()));

        db.cleanup().await?;
        Ok(())
    }

    #[tokio::test]
    async fn test_mark_dirty_triggers_reverification() -> Result<()> {
        let db = TestDb::new().await?;

        let dsl = format!(
            r#"
            (cbu.create :name "{}" :as @cbu)
            (entity.create-limited-company :name "{}" :as @company)
            (entity.create-proper-person :first-name "Dirty" :last-name "Test" :as @owner)

            ;; Build and verify
            (ubo.allege :cbu-id @cbu :from-entity-id @owner :to-entity-id @company
                        :edge-type "ownership" :percentage 100 :as @edge)
            (document.catalog :cbu-id @cbu :doc-type "SHARE_CERTIFICATE" :title "Cert" :as @doc)
            (ubo.link-proof :edge-id @edge :document-id @doc)
            (ubo.verify :edge-id @edge)

            ;; Mark dirty (simulating document expiry or new information)
            (ubo.mark-dirty :edge-id @edge :reason "Document expired")
        "#,
            db.name("DirtyCBU"),
            db.name("DirtyCompany")
        );

        let ctx = db.execute_dsl(&dsl).await?;
        let cbu_id = ctx.resolve("cbu").expect("cbu should be bound");

        // Edge should now need re-verification (back to pending)
        let pending = count_edges_by_state(&db.pool, cbu_id, "pending").await?;
        assert_eq!(pending, 1, "Edge should be pending after mark-dirty");

        // Graph should no longer be converged
        let converged = is_converged(&db.pool, cbu_id).await?;
        assert!(!converged, "Graph should not be converged after mark-dirty");

        db.cleanup().await?;
        Ok(())
    }
}
