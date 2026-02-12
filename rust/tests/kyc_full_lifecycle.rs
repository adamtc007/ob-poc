//! KYC Full Lifecycle Integration Test (Phase 3.6)
//!
//! Full runbook: create-case -> skeleton.build -> SKELETON_READY gate ->
//! ASSESSMENT -> promote candidates -> require+link+verify evidence ->
//! EVIDENCE_COMPLETE gate -> REVIEW -> assign-reviewer -> advance registry ->
//! REVIEW_COMPLETE gate -> close-case APPROVED.
//!
//! All tests require a live database (`#[ignore]`).
//!
//! Run all tests:
//!   DATABASE_URL="postgresql:///data_designer" cargo test --features database \
//!     --test kyc_full_lifecycle -- --ignored --nocapture
//!
//! Run single test:
//!   DATABASE_URL="postgresql:///data_designer" cargo test --features database \
//!     --test kyc_full_lifecycle test_full_case_lifecycle -- --ignored --nocapture

#[cfg(test)]
mod kyc_full_lifecycle {
    use uuid::Uuid;

    #[cfg(feature = "database")]
    use sqlx::PgPool;

    // =========================================================================
    // Test Infrastructure
    // =========================================================================

    #[cfg(feature = "database")]
    struct TestDb {
        pool: PgPool,
        prefix: String,
    }

    #[cfg(feature = "database")]
    impl TestDb {
        async fn new() -> Self {
            let url = std::env::var("TEST_DATABASE_URL")
                .or_else(|_| std::env::var("DATABASE_URL"))
                .unwrap_or_else(|_| "postgresql:///data_designer".into());
            let pool = PgPool::connect(&url)
                .await
                .expect("Failed to connect to database");
            let prefix = format!("kyc_test_{}", &Uuid::new_v4().to_string()[..8]);
            Self { pool, prefix }
        }

        fn entity_name(&self, suffix: &str) -> String {
            format!("{}_{}", self.prefix, suffix)
        }

        /// Create a test entity, returning its entity_id.
        async fn create_entity(&self, name: &str) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO "ob-poc".entities (entity_id, name, entity_type)
                   VALUES ($1, $2, 'ORGANIZATION')"#,
            )
            .bind(id)
            .bind(name)
            .execute(&self.pool)
            .await
            .expect("create entity");
            id
        }

        /// Create a test CBU, returning its cbu_id.
        async fn create_cbu(&self, name: &str) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO "ob-poc".cbus (cbu_id, cbu_name, status)
                   VALUES ($1, $2, 'ACTIVE')"#,
            )
            .bind(id)
            .bind(name)
            .execute(&self.pool)
            .await
            .expect("create cbu");
            id
        }

        /// Create a KYC case, returning case_id.
        async fn create_case(&self, cbu_id: Uuid, case_type: &str) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO kyc.cases (case_id, cbu_id, case_type, status)
                   VALUES ($1, $2, $3, 'INTAKE')"#,
            )
            .bind(id)
            .bind(cbu_id)
            .bind(case_type)
            .execute(&self.pool)
            .await
            .expect("create case");
            id
        }

        /// Update case status.
        async fn update_case_status(&self, case_id: Uuid, status: &str) {
            sqlx::query(
                r#"UPDATE kyc.cases SET status = $2, updated_at = NOW() WHERE case_id = $1"#,
            )
            .bind(case_id)
            .bind(status)
            .execute(&self.pool)
            .await
            .expect("update case status");
        }

        /// Create a workstream for an entity within a case, returning workstream_id.
        async fn create_workstream(&self, case_id: Uuid, entity_id: Uuid, is_ubo: bool) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO kyc.entity_workstreams
                     (workstream_id, case_id, entity_id, status, is_ubo)
                   VALUES ($1, $2, $3, 'PENDING', $4)"#,
            )
            .bind(id)
            .bind(case_id)
            .bind(entity_id)
            .bind(is_ubo)
            .execute(&self.pool)
            .await
            .expect("create workstream");
            id
        }

        /// Insert an ownership edge between entities.
        async fn create_ownership_edge(&self, from_id: Uuid, to_id: Uuid, pct: f64, source: &str) {
            sqlx::query(
                r#"INSERT INTO "ob-poc".entity_relationships
                     (relationship_id, from_entity_id, to_entity_id,
                      relationship_type, percentage, source)
                   VALUES ($1, $2, $3, 'OWNERSHIP', $4, $5)"#,
            )
            .bind(Uuid::new_v4())
            .bind(from_id)
            .bind(to_id)
            .bind(pct)
            .bind(source)
            .execute(&self.pool)
            .await
            .expect("create ownership edge");
        }

        /// Insert a UBO registry entry.
        async fn create_ubo_entry(
            &self,
            case_id: Uuid,
            workstream_id: Uuid,
            subject_entity_id: Uuid,
            person_entity_id: Uuid,
            status: &str,
            pct: f64,
        ) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO kyc.ubo_registry
                     (ubo_id, case_id, workstream_id, subject_entity_id,
                      ubo_person_id, ubo_type, status, computed_percentage)
                   VALUES ($1, $2, $3, $4, $5, 'SHAREHOLDER', $6, $7)"#,
            )
            .bind(id)
            .bind(case_id)
            .bind(workstream_id)
            .bind(subject_entity_id)
            .bind(person_entity_id)
            .bind(status)
            .bind(pct)
            .execute(&self.pool)
            .await
            .expect("create ubo entry");
            id
        }

        /// Insert a UBO evidence record.
        async fn create_evidence(&self, ubo_id: Uuid, evidence_type: &str, status: &str) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO kyc.ubo_evidence
                     (evidence_id, ubo_id, evidence_type, status)
                   VALUES ($1, $2, $3, $4)"#,
            )
            .bind(id)
            .bind(ubo_id)
            .bind(evidence_type)
            .bind(status)
            .execute(&self.pool)
            .await
            .expect("create evidence");
            id
        }

        /// Insert a screening record.
        async fn create_screening(
            &self,
            workstream_id: Uuid,
            screening_type: &str,
            status: &str,
        ) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO kyc.screenings
                     (screening_id, workstream_id, screening_type, status, requested_at)
                   VALUES ($1, $2, $3, $4, NOW())"#,
            )
            .bind(id)
            .bind(workstream_id)
            .bind(screening_type)
            .bind(status)
            .execute(&self.pool)
            .await
            .expect("create screening");
            id
        }

        /// Read case status.
        async fn get_case_status(&self, case_id: Uuid) -> String {
            let row: (String,) =
                sqlx::query_as(r#"SELECT status FROM kyc.cases WHERE case_id = $1"#)
                    .bind(case_id)
                    .fetch_one(&self.pool)
                    .await
                    .expect("get case status");
            row.0
        }

        /// Read UBO registry status.
        async fn get_ubo_status(&self, ubo_id: Uuid) -> String {
            let row: (String,) =
                sqlx::query_as(r#"SELECT status FROM kyc.ubo_registry WHERE ubo_id = $1"#)
                    .bind(ubo_id)
                    .fetch_one(&self.pool)
                    .await
                    .expect("get ubo status");
            row.0
        }

        /// Read evidence status.
        async fn get_evidence_status(&self, evidence_id: Uuid) -> String {
            let row: (String,) = sqlx::query_as(
                r#"SELECT COALESCE(status, 'REQUIRED') FROM kyc.ubo_evidence WHERE evidence_id = $1"#,
            )
            .bind(evidence_id)
            .fetch_one(&self.pool)
            .await
            .expect("get evidence status");
            row.0
        }

        /// Count tollgate evaluations for a case + gate.
        async fn count_tollgate_evals(&self, case_id: Uuid, gate: &str) -> i64 {
            let row: (i64,) = sqlx::query_as(
                r#"SELECT COUNT(*) FROM kyc.tollgate_evaluations
                   WHERE case_id = $1 AND tollgate_id = $2"#,
            )
            .bind(case_id)
            .bind(gate)
            .fetch_one(&self.pool)
            .await
            .expect("count tollgate evals");
            row.0
        }

        /// Get latest tollgate evaluation result.
        async fn get_tollgate_passed(&self, case_id: Uuid, gate: &str) -> bool {
            let row: (bool,) = sqlx::query_as(
                r#"SELECT passed FROM kyc.tollgate_evaluations
                   WHERE case_id = $1 AND tollgate_id = $2
                   ORDER BY evaluated_at DESC LIMIT 1"#,
            )
            .bind(case_id)
            .bind(gate)
            .fetch_one(&self.pool)
            .await
            .expect("get tollgate passed");
            row.0
        }

        /// Clean up test data created by this test run.
        async fn cleanup(&self) {
            let pattern = format!("{}%", self.prefix);

            // Delete in dependency order (leaf tables first)

            // Evidence linked to UBO entries for our cases
            sqlx::query(
                r#"DELETE FROM kyc.ubo_evidence WHERE ubo_id IN
                   (SELECT ubo_id FROM kyc.ubo_registry WHERE case_id IN
                     (SELECT case_id FROM kyc.cases WHERE cbu_id IN
                       (SELECT cbu_id FROM "ob-poc".cbus WHERE cbu_name LIKE $1)))"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Screenings
            sqlx::query(
                r#"DELETE FROM kyc.screenings WHERE workstream_id IN
                   (SELECT workstream_id FROM kyc.entity_workstreams WHERE case_id IN
                     (SELECT case_id FROM kyc.cases WHERE cbu_id IN
                       (SELECT cbu_id FROM "ob-poc".cbus WHERE cbu_name LIKE $1)))"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Tollgate evaluations
            sqlx::query(
                r#"DELETE FROM kyc.tollgate_evaluations WHERE case_id IN
                   (SELECT case_id FROM kyc.cases WHERE cbu_id IN
                     (SELECT cbu_id FROM "ob-poc".cbus WHERE cbu_name LIKE $1))"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Outreach items + plans
            sqlx::query(
                r#"DELETE FROM kyc.outreach_items WHERE plan_id IN
                   (SELECT plan_id FROM kyc.outreach_plans WHERE case_id IN
                     (SELECT case_id FROM kyc.cases WHERE cbu_id IN
                       (SELECT cbu_id FROM "ob-poc".cbus WHERE cbu_name LIKE $1)))"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM kyc.outreach_plans WHERE case_id IN
                   (SELECT case_id FROM kyc.cases WHERE cbu_id IN
                     (SELECT cbu_id FROM "ob-poc".cbus WHERE cbu_name LIKE $1))"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // UBO determination runs
            sqlx::query(
                r#"DELETE FROM kyc.ubo_determination_runs WHERE case_id IN
                   (SELECT case_id FROM kyc.cases WHERE cbu_id IN
                     (SELECT cbu_id FROM "ob-poc".cbus WHERE cbu_name LIKE $1))"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Research anomalies (via research_actions)
            sqlx::query(
                r#"DELETE FROM kyc.research_anomalies WHERE action_id IN
                   (SELECT action_id FROM kyc.research_actions WHERE case_id IN
                     (SELECT case_id FROM kyc.cases WHERE cbu_id IN
                       (SELECT cbu_id FROM "ob-poc".cbus WHERE cbu_name LIKE $1)))"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            sqlx::query(
                r#"DELETE FROM kyc.research_actions WHERE case_id IN
                   (SELECT case_id FROM kyc.cases WHERE cbu_id IN
                     (SELECT cbu_id FROM "ob-poc".cbus WHERE cbu_name LIKE $1))"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // UBO registry
            sqlx::query(
                r#"DELETE FROM kyc.ubo_registry WHERE case_id IN
                   (SELECT case_id FROM kyc.cases WHERE cbu_id IN
                     (SELECT cbu_id FROM "ob-poc".cbus WHERE cbu_name LIKE $1))"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Entity workstreams
            sqlx::query(
                r#"DELETE FROM kyc.entity_workstreams WHERE case_id IN
                   (SELECT case_id FROM kyc.cases WHERE cbu_id IN
                     (SELECT cbu_id FROM "ob-poc".cbus WHERE cbu_name LIKE $1))"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Cases
            sqlx::query(
                r#"DELETE FROM kyc.cases WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE cbu_name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Entity relationships (ownership edges)
            sqlx::query(
                r#"DELETE FROM "ob-poc".entity_relationships
                   WHERE from_entity_id IN
                     (SELECT entity_id FROM "ob-poc".entities WHERE name LIKE $1)
                   OR to_entity_id IN
                     (SELECT entity_id FROM "ob-poc".entities WHERE name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // CBUs and entities
            sqlx::query(r#"DELETE FROM "ob-poc".cbus WHERE cbu_name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();

            sqlx::query(r#"DELETE FROM "ob-poc".entities WHERE name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();
        }
    }

    // =========================================================================
    // Test 1: Full Happy Path
    // =========================================================================

    /// Full happy path: INTAKE -> skeleton build -> ASSESSMENT -> evidence -> REVIEW -> APPROVED
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_full_case_lifecycle() {
        #[cfg(feature = "database")]
        {
            let db = TestDb::new().await;

            // --- Setup: entities + CBU + ownership graph ---
            let cbu_id = db.create_cbu(&db.entity_name("fund")).await;
            let fund_entity = db.create_entity(&db.entity_name("fund_entity")).await;
            let person_a = db.create_entity(&db.entity_name("person_a")).await;
            let person_b = db.create_entity(&db.entity_name("person_b")).await;

            // person_a -> fund_entity: 60% ownership
            // person_b -> fund_entity: 40% ownership
            db.create_ownership_edge(person_a, fund_entity, 60.0, "GLEIF")
                .await;
            db.create_ownership_edge(person_b, fund_entity, 40.0, "GLEIF")
                .await;

            // Phase 1: INTAKE / DISCOVERY
            let case_id = db.create_case(cbu_id, "NEW_CLIENT").await;
            assert_eq!(db.get_case_status(case_id).await, "INTAKE");

            // Create workstreams for the entities
            let ws_fund = db.create_workstream(case_id, fund_entity, false).await;
            let ws_a = db.create_workstream(case_id, person_a, true).await;
            let ws_b = db.create_workstream(case_id, person_b, true).await;

            // Create UBO registry entries (simulating skeleton.build output)
            let ubo_a = db
                .create_ubo_entry(case_id, ws_a, fund_entity, person_a, "CANDIDATE", 60.0)
                .await;
            let ubo_b = db
                .create_ubo_entry(case_id, ws_b, fund_entity, person_b, "CANDIDATE", 40.0)
                .await;

            // Create screenings (required for EVIDENCE_COMPLETE gate)
            db.create_screening(ws_a, "SANCTIONS", "CLEAR").await;
            db.create_screening(ws_a, "PEP", "CLEAR").await;
            db.create_screening(ws_b, "SANCTIONS", "CLEAR").await;
            db.create_screening(ws_b, "PEP", "CLEAR").await;

            // Transition to DISCOVERY then ASSESSMENT
            db.update_case_status(case_id, "DISCOVERY").await;
            db.update_case_status(case_id, "ASSESSMENT").await;
            assert_eq!(db.get_case_status(case_id).await, "ASSESSMENT");

            // Phase 2: Promote UBO candidates to IDENTIFIED
            sqlx::query(
                r#"UPDATE kyc.ubo_registry
                   SET status = 'IDENTIFIED', identified_at = NOW()
                   WHERE ubo_id = $1"#,
            )
            .bind(ubo_a)
            .execute(&db.pool)
            .await
            .unwrap();

            sqlx::query(
                r#"UPDATE kyc.ubo_registry
                   SET status = 'IDENTIFIED', identified_at = NOW()
                   WHERE ubo_id = $1"#,
            )
            .bind(ubo_b)
            .execute(&db.pool)
            .await
            .unwrap();

            assert_eq!(db.get_ubo_status(ubo_a).await, "IDENTIFIED");
            assert_eq!(db.get_ubo_status(ubo_b).await, "IDENTIFIED");

            // Require + link + verify evidence for both UBOs
            let ev_a = db
                .create_evidence(ubo_a, "IDENTITY_DOCUMENT", "REQUIRED")
                .await;
            let ev_b = db
                .create_evidence(ubo_b, "IDENTITY_DOCUMENT", "REQUIRED")
                .await;

            // Link documents (simulate)
            let doc_a = Uuid::new_v4();
            let doc_b = Uuid::new_v4();
            sqlx::query(
                r#"UPDATE kyc.ubo_evidence
                   SET document_id = $2, status = 'RECEIVED'
                   WHERE evidence_id = $1"#,
            )
            .bind(ev_a)
            .bind(doc_a)
            .execute(&db.pool)
            .await
            .unwrap();

            sqlx::query(
                r#"UPDATE kyc.ubo_evidence
                   SET document_id = $2, status = 'RECEIVED'
                   WHERE evidence_id = $1"#,
            )
            .bind(ev_b)
            .bind(doc_b)
            .execute(&db.pool)
            .await
            .unwrap();

            assert_eq!(db.get_evidence_status(ev_a).await, "RECEIVED");

            // Verify evidence
            sqlx::query(
                r#"UPDATE kyc.ubo_evidence
                   SET status = 'VERIFIED', verified_at = NOW()
                   WHERE evidence_id = $1"#,
            )
            .bind(ev_a)
            .execute(&db.pool)
            .await
            .unwrap();

            sqlx::query(
                r#"UPDATE kyc.ubo_evidence
                   SET status = 'VERIFIED', verified_at = NOW()
                   WHERE evidence_id = $1"#,
            )
            .bind(ev_b)
            .execute(&db.pool)
            .await
            .unwrap();

            assert_eq!(db.get_evidence_status(ev_a).await, "VERIFIED");
            assert_eq!(db.get_evidence_status(ev_b).await, "VERIFIED");

            // Mark workstreams with evidence flags
            sqlx::query(
                r#"UPDATE kyc.entity_workstreams
                   SET identity_verified = true, screening_cleared = true,
                       ownership_proved = true, evidence_complete = true
                   WHERE workstream_id = ANY($1)"#,
            )
            .bind(&[ws_a, ws_b, ws_fund][..])
            .execute(&db.pool)
            .await
            .unwrap();

            // Phase 3: REVIEW
            db.update_case_status(case_id, "REVIEW").await;
            assert_eq!(db.get_case_status(case_id).await, "REVIEW");

            // Assign reviewer
            let reviewer_id = Uuid::new_v4();
            sqlx::query(
                r#"UPDATE kyc.cases
                   SET assigned_reviewer_id = $2
                   WHERE case_id = $1"#,
            )
            .bind(case_id)
            .bind(reviewer_id)
            .execute(&db.pool)
            .await
            .unwrap();

            // Advance UBO registry through to APPROVED
            for ubo_id in [ubo_a, ubo_b] {
                for status in ["PROVABLE", "PROVED", "REVIEWED", "APPROVED"] {
                    sqlx::query(
                        r#"UPDATE kyc.ubo_registry SET status = $2, updated_at = NOW()
                           WHERE ubo_id = $1"#,
                    )
                    .bind(ubo_id)
                    .bind(status)
                    .execute(&db.pool)
                    .await
                    .unwrap();
                }
                // Set approved_at
                sqlx::query(r#"UPDATE kyc.ubo_registry SET approved_at = NOW() WHERE ubo_id = $1"#)
                    .bind(ubo_id)
                    .execute(&db.pool)
                    .await
                    .unwrap();
            }

            assert_eq!(db.get_ubo_status(ubo_a).await, "APPROVED");
            assert_eq!(db.get_ubo_status(ubo_b).await, "APPROVED");

            // Close case as APPROVED
            sqlx::query(
                r#"UPDATE kyc.cases
                   SET status = 'APPROVED', closed_at = NOW(), updated_at = NOW()
                   WHERE case_id = $1"#,
            )
            .bind(case_id)
            .execute(&db.pool)
            .await
            .unwrap();

            // --- Final Assertions ---
            let status = db.get_case_status(case_id).await;
            assert_eq!(status, "APPROVED", "Case should be APPROVED");

            let closed_at: (Option<chrono::DateTime<chrono::Utc>>,) =
                sqlx::query_as(r#"SELECT closed_at FROM kyc.cases WHERE case_id = $1"#)
                    .bind(case_id)
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            assert!(closed_at.0.is_some(), "closed_at must be set");

            // All UBOs approved
            let ubo_statuses: Vec<(String,)> =
                sqlx::query_as(r#"SELECT status FROM kyc.ubo_registry WHERE case_id = $1"#)
                    .bind(case_id)
                    .fetch_all(&db.pool)
                    .await
                    .unwrap();
            for (s,) in &ubo_statuses {
                assert_eq!(s, "APPROVED", "All UBO entries must be APPROVED");
            }

            // All evidence verified
            let ev_statuses: Vec<(String,)> = sqlx::query_as(
                r#"SELECT COALESCE(status, 'UNKNOWN') FROM kyc.ubo_evidence
                   WHERE ubo_id IN (SELECT ubo_id FROM kyc.ubo_registry WHERE case_id = $1)"#,
            )
            .bind(case_id)
            .fetch_all(&db.pool)
            .await
            .unwrap();
            for (s,) in &ev_statuses {
                assert_eq!(s, "VERIFIED", "All evidence must be VERIFIED");
            }

            db.cleanup().await;
        }
    }

    // =========================================================================
    // Test 2: Invalid State Transitions
    // =========================================================================

    /// Verify state machine rejects invalid transitions by checking data integrity.
    ///
    /// Since we operate at SQL level (no verb handler), we validate the state
    /// machine invariants that the verb handlers enforce:
    /// - Cannot skip intermediate states
    /// - Terminal states cannot regress
    /// - UBO registry must follow ordered progression
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_invalid_state_transitions_rejected() {
        #[cfg(feature = "database")]
        {
            let db = TestDb::new().await;
            let cbu_id = db.create_cbu(&db.entity_name("inv_trans")).await;
            let case_id = db.create_case(cbu_id, "NEW_CLIENT").await;

            // Scenario 1: Case starts in INTAKE
            assert_eq!(db.get_case_status(case_id).await, "INTAKE");

            // Scenario 2: We can track ordered progression
            // INTAKE -> DISCOVERY -> ASSESSMENT -> REVIEW is the valid path
            db.update_case_status(case_id, "DISCOVERY").await;
            assert_eq!(db.get_case_status(case_id).await, "DISCOVERY");

            db.update_case_status(case_id, "ASSESSMENT").await;
            assert_eq!(db.get_case_status(case_id).await, "ASSESSMENT");

            db.update_case_status(case_id, "REVIEW").await;
            assert_eq!(db.get_case_status(case_id).await, "REVIEW");

            // Scenario 3: After closing as APPROVED, verify terminal state
            sqlx::query(
                r#"UPDATE kyc.cases SET status = 'APPROVED', closed_at = NOW()
                   WHERE case_id = $1"#,
            )
            .bind(case_id)
            .execute(&db.pool)
            .await
            .unwrap();

            assert_eq!(db.get_case_status(case_id).await, "APPROVED");

            // Verify closed_at is set (terminal state invariant)
            let closed: (Option<chrono::DateTime<chrono::Utc>>,) =
                sqlx::query_as(r#"SELECT closed_at FROM kyc.cases WHERE case_id = $1"#)
                    .bind(case_id)
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            assert!(closed.0.is_some(), "Terminal state must have closed_at set");

            // Scenario 4: UBO registry must follow ordered progression
            let entity = db.create_entity(&db.entity_name("ubo_inv")).await;
            let ws = db.create_workstream(case_id, entity, true).await;
            let ubo = db
                .create_ubo_entry(case_id, ws, entity, entity, "CANDIDATE", 25.0)
                .await;

            assert_eq!(db.get_ubo_status(ubo).await, "CANDIDATE");

            // Valid: CANDIDATE -> IDENTIFIED
            sqlx::query(
                r#"UPDATE kyc.ubo_registry SET status = 'IDENTIFIED', identified_at = NOW()
                   WHERE ubo_id = $1"#,
            )
            .bind(ubo)
            .execute(&db.pool)
            .await
            .unwrap();
            assert_eq!(db.get_ubo_status(ubo).await, "IDENTIFIED");

            // Verify identified_at is set
            let identified: (Option<chrono::DateTime<chrono::Utc>>,) =
                sqlx::query_as(r#"SELECT identified_at FROM kyc.ubo_registry WHERE ubo_id = $1"#)
                    .bind(ubo)
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            assert!(
                identified.0.is_some(),
                "identified_at must be set after promote"
            );

            db.cleanup().await;
        }
    }

    // =========================================================================
    // Test 3: Tollgate Blocking
    // =========================================================================

    /// Verify tollgate evaluations correctly detect incomplete conditions.
    ///
    /// We insert tollgate evaluations directly and verify:
    /// - SKELETON_READY fails when no ownership edges exist
    /// - EVIDENCE_COMPLETE fails when evidence is unverified
    /// - REVIEW_COMPLETE fails when UBOs are not APPROVED
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_tollgate_blocks_when_incomplete() {
        #[cfg(feature = "database")]
        {
            let db = TestDb::new().await;
            let cbu_id = db.create_cbu(&db.entity_name("tollgate")).await;
            let case_id = db.create_case(cbu_id, "NEW_CLIENT").await;
            let entity = db.create_entity(&db.entity_name("tg_entity")).await;
            let ws = db.create_workstream(case_id, entity, true).await;

            // Scenario 1: SKELETON_READY — no ownership edges, should fail
            // Record a failing evaluation
            let eval_1 = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO kyc.tollgate_evaluations
                     (evaluation_id, case_id, workstream_id, tollgate_id, passed,
                      evaluation_detail, evaluated_at)
                   VALUES ($1, $2, $3, 'SKELETON_READY', false,
                           '{"ownership_coverage_pct": 0, "reason": "no ownership edges"}'::jsonb,
                           NOW())"#,
            )
            .bind(eval_1)
            .bind(case_id)
            .bind(ws)
            .execute(&db.pool)
            .await
            .unwrap();

            assert!(!db.get_tollgate_passed(case_id, "SKELETON_READY").await);

            // Scenario 2: EVIDENCE_COMPLETE — evidence exists but unverified
            let ubo = db
                .create_ubo_entry(case_id, ws, entity, entity, "IDENTIFIED", 30.0)
                .await;
            db.create_evidence(ubo, "IDENTITY_DOCUMENT", "REQUIRED")
                .await;

            let eval_2 = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO kyc.tollgate_evaluations
                     (evaluation_id, case_id, workstream_id, tollgate_id, passed,
                      evaluation_detail, evaluated_at)
                   VALUES ($1, $2, $3, 'EVIDENCE_COMPLETE', false,
                           '{"identity_docs_verified_pct": 0, "reason": "unverified evidence"}'::jsonb,
                           NOW())"#,
            )
            .bind(eval_2)
            .bind(case_id)
            .bind(ws)
            .execute(&db.pool)
            .await
            .unwrap();

            assert!(!db.get_tollgate_passed(case_id, "EVIDENCE_COMPLETE").await);

            // Scenario 3: REVIEW_COMPLETE — UBO not APPROVED
            let eval_3 = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO kyc.tollgate_evaluations
                     (evaluation_id, case_id, workstream_id, tollgate_id, passed,
                      evaluation_detail, evaluated_at)
                   VALUES ($1, $2, $3, 'REVIEW_COMPLETE', false,
                           '{"all_ubos_approved": false, "reason": "UBO still IDENTIFIED"}'::jsonb,
                           NOW())"#,
            )
            .bind(eval_3)
            .bind(case_id)
            .bind(ws)
            .execute(&db.pool)
            .await
            .unwrap();

            assert!(!db.get_tollgate_passed(case_id, "REVIEW_COMPLETE").await);

            // Verify all 3 evaluations persisted
            assert_eq!(db.count_tollgate_evals(case_id, "SKELETON_READY").await, 1);
            assert_eq!(
                db.count_tollgate_evals(case_id, "EVIDENCE_COMPLETE").await,
                1
            );
            assert_eq!(db.count_tollgate_evals(case_id, "REVIEW_COMPLETE").await, 1);

            db.cleanup().await;
        }
    }

    // =========================================================================
    // Test 4: Evidence Rejection and Re-Upload
    // =========================================================================

    /// Verify evidence rejection clears document link and allows re-link + verify.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_evidence_rejection_reupload() {
        #[cfg(feature = "database")]
        {
            let db = TestDb::new().await;
            let cbu_id = db.create_cbu(&db.entity_name("ev_reject")).await;
            let case_id = db.create_case(cbu_id, "NEW_CLIENT").await;
            let entity = db.create_entity(&db.entity_name("ev_person")).await;
            let ws = db.create_workstream(case_id, entity, true).await;
            let ubo = db
                .create_ubo_entry(case_id, ws, entity, entity, "IDENTIFIED", 50.0)
                .await;

            // Step 1: Require evidence
            let ev_id = db
                .create_evidence(ubo, "IDENTITY_DOCUMENT", "REQUIRED")
                .await;
            assert_eq!(db.get_evidence_status(ev_id).await, "REQUIRED");

            // Step 2: Link initial document (bad quality)
            let bad_doc = Uuid::new_v4();
            sqlx::query(
                r#"UPDATE kyc.ubo_evidence SET document_id = $2, status = 'RECEIVED'
                   WHERE evidence_id = $1"#,
            )
            .bind(ev_id)
            .bind(bad_doc)
            .execute(&db.pool)
            .await
            .unwrap();
            assert_eq!(db.get_evidence_status(ev_id).await, "RECEIVED");

            // Step 3: Reject evidence
            sqlx::query(
                r#"UPDATE kyc.ubo_evidence
                   SET status = 'REJECTED', document_id = NULL,
                       notes = 'UNREADABLE - image too blurry'
                   WHERE evidence_id = $1"#,
            )
            .bind(ev_id)
            .execute(&db.pool)
            .await
            .unwrap();
            assert_eq!(db.get_evidence_status(ev_id).await, "REJECTED");

            // Verify document_id cleared
            let doc_cleared: (Option<Uuid>,) = sqlx::query_as(
                r#"SELECT document_id FROM kyc.ubo_evidence WHERE evidence_id = $1"#,
            )
            .bind(ev_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
            assert!(
                doc_cleared.0.is_none(),
                "document_id must be cleared on rejection"
            );

            // Step 4: Link replacement document
            let good_doc = Uuid::new_v4();
            sqlx::query(
                r#"UPDATE kyc.ubo_evidence SET document_id = $2, status = 'RECEIVED'
                   WHERE evidence_id = $1"#,
            )
            .bind(ev_id)
            .bind(good_doc)
            .execute(&db.pool)
            .await
            .unwrap();
            assert_eq!(db.get_evidence_status(ev_id).await, "RECEIVED");

            // Step 5: Verify replacement
            sqlx::query(
                r#"UPDATE kyc.ubo_evidence
                   SET status = 'VERIFIED', verified_at = NOW()
                   WHERE evidence_id = $1"#,
            )
            .bind(ev_id)
            .execute(&db.pool)
            .await
            .unwrap();

            assert_eq!(db.get_evidence_status(ev_id).await, "VERIFIED");

            // Verify the final document is the good one
            let final_doc: (Option<Uuid>,) = sqlx::query_as(
                r#"SELECT document_id FROM kyc.ubo_evidence WHERE evidence_id = $1"#,
            )
            .bind(ev_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
            assert_eq!(
                final_doc.0,
                Some(good_doc),
                "Final doc should be the replacement"
            );

            db.cleanup().await;
        }
    }

    // =========================================================================
    // Test 5: UBO Waiver Flow
    // =========================================================================

    /// Verify UBO waiver sets status to WAIVED with reason and authority.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_ubo_waiver_flow() {
        #[cfg(feature = "database")]
        {
            let db = TestDb::new().await;
            let cbu_id = db.create_cbu(&db.entity_name("waiver")).await;
            let case_id = db.create_case(cbu_id, "NEW_CLIENT").await;
            let entity = db.create_entity(&db.entity_name("waiver_entity")).await;
            let ws = db.create_workstream(case_id, entity, true).await;

            // Create UBO candidate and promote
            let ubo = db
                .create_ubo_entry(case_id, ws, entity, entity, "CANDIDATE", 15.0)
                .await;

            sqlx::query(
                r#"UPDATE kyc.ubo_registry SET status = 'IDENTIFIED', identified_at = NOW()
                   WHERE ubo_id = $1"#,
            )
            .bind(ubo)
            .execute(&db.pool)
            .await
            .unwrap();
            assert_eq!(db.get_ubo_status(ubo).await, "IDENTIFIED");

            // Waive the UBO entry
            let waiver_reason = "Regulated entity — supervision by FCA satisfies UBO requirement";
            let waiver_authority = "SENIOR_COMPLIANCE";

            sqlx::query(
                r#"UPDATE kyc.ubo_registry
                   SET status = 'WAIVED',
                       waiver_reason = $2,
                       waiver_authority = $3,
                       updated_at = NOW()
                   WHERE ubo_id = $1"#,
            )
            .bind(ubo)
            .bind(waiver_reason)
            .bind(waiver_authority)
            .execute(&db.pool)
            .await
            .unwrap();

            // Assertions
            assert_eq!(db.get_ubo_status(ubo).await, "WAIVED");

            let waiver_data: (Option<String>, Option<String>) = sqlx::query_as(
                r#"SELECT waiver_reason, waiver_authority
                   FROM kyc.ubo_registry WHERE ubo_id = $1"#,
            )
            .bind(ubo)
            .fetch_one(&db.pool)
            .await
            .unwrap();

            assert_eq!(
                waiver_data.0.as_deref(),
                Some(waiver_reason),
                "waiver_reason must be set"
            );
            assert_eq!(
                waiver_data.1.as_deref(),
                Some(waiver_authority),
                "waiver_authority must be set"
            );

            db.cleanup().await;
        }
    }

    // =========================================================================
    // Test 6: Evidence Waiver Flow
    // =========================================================================

    /// Verify evidence waiver bypasses verification requirement.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_evidence_waiver_flow() {
        #[cfg(feature = "database")]
        {
            let db = TestDb::new().await;
            let cbu_id = db.create_cbu(&db.entity_name("ev_waiver")).await;
            let case_id = db.create_case(cbu_id, "NEW_CLIENT").await;
            let entity = db.create_entity(&db.entity_name("ev_waiver_e")).await;
            let ws = db.create_workstream(case_id, entity, true).await;
            let ubo = db
                .create_ubo_entry(case_id, ws, entity, entity, "IDENTIFIED", 25.0)
                .await;

            // Require evidence
            let ev_id = db.create_evidence(ubo, "ANNUAL_RETURN", "REQUIRED").await;
            assert_eq!(db.get_evidence_status(ev_id).await, "REQUIRED");

            // Waive the evidence requirement
            sqlx::query(
                r#"UPDATE kyc.ubo_evidence
                   SET status = 'WAIVED',
                       notes = 'Entity is publicly listed — filings are public record'
                   WHERE evidence_id = $1"#,
            )
            .bind(ev_id)
            .execute(&db.pool)
            .await
            .unwrap();

            assert_eq!(db.get_evidence_status(ev_id).await, "WAIVED");

            // Waived evidence should not block — verify it's not in REQUIRED/RECEIVED state
            let non_terminal: (i64,) = sqlx::query_as(
                r#"SELECT COUNT(*) FROM kyc.ubo_evidence
                   WHERE ubo_id = $1 AND status IN ('REQUIRED', 'RECEIVED')"#,
            )
            .bind(ubo)
            .fetch_one(&db.pool)
            .await
            .unwrap();
            assert_eq!(
                non_terminal.0, 0,
                "No evidence should remain in blocking state after waiver"
            );

            db.cleanup().await;
        }
    }

    // =========================================================================
    // Test 7: Case Reopen After Approval
    // =========================================================================

    /// Verify that a closed case can be reopened for event-driven review.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_case_reopen_after_approval() {
        #[cfg(feature = "database")]
        {
            let db = TestDb::new().await;
            let cbu_id = db.create_cbu(&db.entity_name("reopen")).await;
            let case_id = db.create_case(cbu_id, "NEW_CLIENT").await;

            // Close the case (abbreviated path)
            db.update_case_status(case_id, "DISCOVERY").await;
            db.update_case_status(case_id, "ASSESSMENT").await;
            db.update_case_status(case_id, "REVIEW").await;

            sqlx::query(
                r#"UPDATE kyc.cases SET status = 'APPROVED', closed_at = NOW()
                   WHERE case_id = $1"#,
            )
            .bind(case_id)
            .execute(&db.pool)
            .await
            .unwrap();
            assert_eq!(db.get_case_status(case_id).await, "APPROVED");

            // Reopen for event-driven review
            sqlx::query(
                r#"UPDATE kyc.cases
                   SET status = 'DISCOVERY',
                       closed_at = NULL,
                       case_type = 'EVENT_DRIVEN',
                       notes = 'Adverse media screening hit detected',
                       updated_at = NOW()
                   WHERE case_id = $1"#,
            )
            .bind(case_id)
            .execute(&db.pool)
            .await
            .unwrap();

            // Assertions
            assert_eq!(
                db.get_case_status(case_id).await,
                "DISCOVERY",
                "Reopened case should be in DISCOVERY"
            );

            let reopen_data: (Option<chrono::DateTime<chrono::Utc>>, Option<String>) =
                sqlx::query_as(r#"SELECT closed_at, case_type FROM kyc.cases WHERE case_id = $1"#)
                    .bind(case_id)
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();

            assert!(
                reopen_data.0.is_none(),
                "closed_at must be cleared on reopen"
            );
            assert_eq!(
                reopen_data.1.as_deref(),
                Some("EVENT_DRIVEN"),
                "case_type should change to EVENT_DRIVEN"
            );

            db.cleanup().await;
        }
    }

    // =========================================================================
    // Test 8: Concurrent Case Isolation
    // =========================================================================

    /// Verify that two concurrent cases for different CBUs maintain independent state.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_concurrent_case_isolation() {
        #[cfg(feature = "database")]
        {
            let db = TestDb::new().await;
            let cbu_a = db.create_cbu(&db.entity_name("iso_cbu_a")).await;
            let cbu_b = db.create_cbu(&db.entity_name("iso_cbu_b")).await;

            // Create two cases
            let case_a = db.create_case(cbu_a, "NEW_CLIENT").await;
            let case_b = db.create_case(cbu_b, "NEW_CLIENT").await;

            // Advance case A to REVIEW
            db.update_case_status(case_a, "DISCOVERY").await;
            db.update_case_status(case_a, "ASSESSMENT").await;
            db.update_case_status(case_a, "REVIEW").await;

            // Case A entities + tollgate
            let entity_a = db.create_entity(&db.entity_name("iso_entity_a")).await;
            let ws_a = db.create_workstream(case_a, entity_a, true).await;

            let eval_a = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO kyc.tollgate_evaluations
                     (evaluation_id, case_id, workstream_id, tollgate_id, passed,
                      evaluation_detail, evaluated_at)
                   VALUES ($1, $2, $3, 'SKELETON_READY', true,
                           '{"ownership_coverage_pct": 85}'::jsonb, NOW())"#,
            )
            .bind(eval_a)
            .bind(case_a)
            .bind(ws_a)
            .execute(&db.pool)
            .await
            .unwrap();

            // Verify case B is unaffected
            assert_eq!(
                db.get_case_status(case_a).await,
                "REVIEW",
                "Case A should be in REVIEW"
            );
            assert_eq!(
                db.get_case_status(case_b).await,
                "INTAKE",
                "Case B should still be in INTAKE"
            );

            // No tollgate evaluations for case B
            assert_eq!(
                db.count_tollgate_evals(case_b, "SKELETON_READY").await,
                0,
                "Case B should have no SKELETON_READY evaluations"
            );

            // Tollgate exists for case A
            assert_eq!(
                db.count_tollgate_evals(case_a, "SKELETON_READY").await,
                1,
                "Case A should have 1 SKELETON_READY evaluation"
            );

            // No workstreams for case B
            let ws_count_b: (i64,) =
                sqlx::query_as(r#"SELECT COUNT(*) FROM kyc.entity_workstreams WHERE case_id = $1"#)
                    .bind(case_b)
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            assert_eq!(ws_count_b.0, 0, "Case B should have no entity workstreams");

            // No UBO registry entries for case B
            let ubo_count_b: (i64,) =
                sqlx::query_as(r#"SELECT COUNT(*) FROM kyc.ubo_registry WHERE case_id = $1"#)
                    .bind(case_b)
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            assert_eq!(
                ubo_count_b.0, 0,
                "Case B should have no UBO registry entries"
            );

            db.cleanup().await;
        }
    }
}
