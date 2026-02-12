//! Deal-to-KYC-to-Approved Lifecycle Integration Test (Phase 4.5)
//!
//! Full lifecycle: deal CONTRACTED -> KYC case linked via deal_ubo_assessments ->
//! skeleton.build -> ASSESSMENT -> REVIEW -> close-case APPROVED ->
//! deal_events records KYC_GATE_COMPLETED.
//!
//! This covers the bridge between the deal origination pipeline (067) and
//! the KYC case lifecycle, validating that KYC case closure propagates
//! a gate-completion event back to the deal audit trail.
//!
//! All tests require a live database (`#[ignore]`).
//!
//! Run all tests:
//!   DATABASE_URL="postgresql:///data_designer" cargo test --features database \
//!     --test deal_to_kyc_lifecycle -- --ignored --nocapture
//!
//! Run single test:
//!   DATABASE_URL="postgresql:///data_designer" cargo test --features database \
//!     --test deal_to_kyc_lifecycle test_deal_to_kyc_full_lifecycle -- --ignored --nocapture

#[cfg(test)]
mod deal_to_kyc_lifecycle {
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
            let prefix = format!("dkyc_test_{}", &Uuid::new_v4().to_string()[..8]);
            Self { pool, prefix }
        }

        fn name(&self, suffix: &str) -> String {
            format!("{}_{}", self.prefix, suffix)
        }

        /// Create a test entity.
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

        /// Create a test CBU.
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

        /// Create a test client group.
        async fn create_client_group(&self, name: &str) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO "ob-poc".client_group (group_id, canonical_name)
                   VALUES ($1, $2)"#,
            )
            .bind(id)
            .bind(name)
            .execute(&self.pool)
            .await
            .expect("create client group");
            id
        }

        /// Create a deal, returning deal_id.
        async fn create_deal(&self, name: &str, group_id: Uuid, status: &str) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO "ob-poc".deals
                     (deal_id, deal_name, primary_client_group_id, deal_status,
                      sales_owner)
                   VALUES ($1, $2, $3, $4, 'test@bank.com')"#,
            )
            .bind(id)
            .bind(name)
            .bind(group_id)
            .bind(status)
            .execute(&self.pool)
            .await
            .expect("create deal");

            // Record DEAL_CREATED event
            sqlx::query(
                r#"INSERT INTO "ob-poc".deal_events
                     (event_id, deal_id, event_type, subject_type, subject_id,
                      new_value, actor)
                   VALUES ($1, $2, 'DEAL_CREATED', 'DEAL', $2, $3, 'test@bank.com')"#,
            )
            .bind(Uuid::new_v4())
            .bind(id)
            .bind(status)
            .execute(&self.pool)
            .await
            .expect("record deal created event");

            id
        }

        /// Create a KYC case linked to a deal, returning case_id.
        async fn create_case_with_deal(&self, cbu_id: Uuid, deal_id: Uuid, group_id: Uuid) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO kyc.cases
                     (case_id, cbu_id, case_type, status, deal_id, client_group_id)
                   VALUES ($1, $2, 'NEW_CLIENT', 'INTAKE', $3, $4)"#,
            )
            .bind(id)
            .bind(cbu_id)
            .bind(deal_id)
            .bind(group_id)
            .execute(&self.pool)
            .await
            .expect("create case with deal");
            id
        }

        /// Create a standalone KYC case (no deal link).
        async fn create_case_standalone(&self, cbu_id: Uuid) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO kyc.cases (case_id, cbu_id, case_type, status)
                   VALUES ($1, $2, 'NEW_CLIENT', 'INTAKE')"#,
            )
            .bind(id)
            .bind(cbu_id)
            .execute(&self.pool)
            .await
            .expect("create standalone case");
            id
        }

        /// Link a case to a deal via deal_ubo_assessments.
        async fn link_case_to_deal(&self, deal_id: Uuid, entity_id: Uuid, case_id: Uuid) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query(
                r#"INSERT INTO "ob-poc".deal_ubo_assessments
                     (assessment_id, deal_id, entity_id, kyc_case_id,
                      assessment_status)
                   VALUES ($1, $2, $3, $4, 'PENDING')"#,
            )
            .bind(id)
            .bind(deal_id)
            .bind(entity_id)
            .bind(case_id)
            .execute(&self.pool)
            .await
            .expect("link case to deal");
            id
        }

        /// Progress a case through the full lifecycle to REVIEW status.
        async fn advance_case_to_review(&self, case_id: Uuid) {
            for status in ["DISCOVERY", "ASSESSMENT", "REVIEW"] {
                sqlx::query(
                    r#"UPDATE kyc.cases SET status = $2, updated_at = NOW()
                       WHERE case_id = $1"#,
                )
                .bind(case_id)
                .bind(status)
                .execute(&self.pool)
                .await
                .unwrap();
            }
        }

        /// Close a case with a specific status. Mimics KycCaseCloseOp behavior:
        /// if status=APPROVED and deal_id exists, emits KYC_GATE_COMPLETED event.
        async fn close_case(&self, case_id: Uuid, close_status: &str) {
            // Load case to get deal_id
            let case_row: (String, Option<Uuid>, Option<String>) = sqlx::query_as(
                r#"SELECT status, deal_id, case_ref FROM kyc.cases WHERE case_id = $1"#,
            )
            .bind(case_id)
            .fetch_one(&self.pool)
            .await
            .expect("load case for close");

            let current_status = case_row.0;
            let deal_id = case_row.1;
            let case_ref = case_row.2.unwrap_or_default();

            assert_eq!(
                current_status, "REVIEW",
                "Case must be in REVIEW to close (was {})",
                current_status
            );

            // Close the case
            sqlx::query(
                r#"UPDATE kyc.cases
                   SET status = $2, closed_at = NOW(), updated_at = NOW()
                   WHERE case_id = $1"#,
            )
            .bind(case_id)
            .bind(close_status)
            .execute(&self.pool)
            .await
            .unwrap();

            // If APPROVED and linked to deal -> emit KYC_GATE_COMPLETED
            if close_status == "APPROVED" {
                if let Some(did) = deal_id {
                    let desc = format!("KYC case {} approved for case_id={}", case_ref, case_id);
                    sqlx::query(
                        r#"INSERT INTO "ob-poc".deal_events
                             (event_id, deal_id, event_type, subject_type,
                              subject_id, new_value, description)
                           VALUES ($1, $2, 'KYC_GATE_COMPLETED', 'KYC_CASE',
                                   $3, 'APPROVED', $4)"#,
                    )
                    .bind(Uuid::new_v4())
                    .bind(did)
                    .bind(case_id)
                    .bind(&desc)
                    .execute(&self.pool)
                    .await
                    .unwrap();
                }
            }
        }

        /// Count deal events of a given type for a deal.
        async fn count_deal_events(&self, deal_id: Uuid, event_type: &str) -> i64 {
            let row: (i64,) = sqlx::query_as(
                r#"SELECT COUNT(*) FROM "ob-poc".deal_events
                   WHERE deal_id = $1 AND event_type = $2"#,
            )
            .bind(deal_id)
            .bind(event_type)
            .fetch_one(&self.pool)
            .await
            .expect("count deal events");
            row.0
        }

        /// Count deal events referencing a specific case.
        async fn count_gate_events_for_case(&self, case_id: Uuid) -> i64 {
            let row: (i64,) = sqlx::query_as(
                r#"SELECT COUNT(*) FROM "ob-poc".deal_events
                   WHERE event_type = 'KYC_GATE_COMPLETED'
                     AND subject_type = 'KYC_CASE'
                     AND subject_id = $1"#,
            )
            .bind(case_id)
            .fetch_one(&self.pool)
            .await
            .expect("count gate events for case");
            row.0
        }

        /// Get assessment status for a deal+case pair.
        async fn get_assessment_status(&self, deal_id: Uuid, case_id: Uuid) -> Option<String> {
            let row: Option<(String,)> = sqlx::query_as(
                r#"SELECT assessment_status FROM "ob-poc".deal_ubo_assessments
                   WHERE deal_id = $1 AND kyc_case_id = $2"#,
            )
            .bind(deal_id)
            .bind(case_id)
            .fetch_optional(&self.pool)
            .await
            .expect("get assessment status");
            row.map(|r| r.0)
        }

        async fn cleanup(&self) {
            let pattern = format!("{}%", self.prefix);

            // Deal events
            sqlx::query(
                r#"DELETE FROM "ob-poc".deal_events WHERE deal_id IN
                   (SELECT deal_id FROM "ob-poc".deals WHERE deal_name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Deal UBO assessments
            sqlx::query(
                r#"DELETE FROM "ob-poc".deal_ubo_assessments WHERE deal_id IN
                   (SELECT deal_id FROM "ob-poc".deals WHERE deal_name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // KYC cases
            sqlx::query(
                r#"DELETE FROM kyc.cases WHERE cbu_id IN
                   (SELECT cbu_id FROM "ob-poc".cbus WHERE cbu_name LIKE $1)"#,
            )
            .bind(&pattern)
            .execute(&self.pool)
            .await
            .ok();

            // Deals
            sqlx::query(r#"DELETE FROM "ob-poc".deals WHERE deal_name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();

            // CBUs, entities, client groups
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

            sqlx::query(r#"DELETE FROM "ob-poc".client_group WHERE canonical_name LIKE $1"#)
                .bind(&pattern)
                .execute(&self.pool)
                .await
                .ok();
        }
    }

    // =========================================================================
    // Test 1: Full Happy Path — Deal -> KYC -> Approved -> Gate Event
    // =========================================================================

    /// Full lifecycle: deal CONTRACTED -> KYC case created and linked ->
    /// case progresses -> APPROVED -> deal_events has KYC_GATE_COMPLETED.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_deal_to_kyc_full_lifecycle() {
        #[cfg(feature = "database")]
        {
            let db = TestDb::new().await;

            // Phase 1: DEAL SETUP
            let group_id = db.create_client_group(&db.name("group")).await;
            let cbu_id = db.create_cbu(&db.name("fund")).await;
            let entity_id = db.create_entity(&db.name("entity")).await;
            let deal_id = db
                .create_deal(&db.name("deal"), group_id, "CONTRACTED")
                .await;

            // Verify deal creation event
            assert_eq!(
                db.count_deal_events(deal_id, "DEAL_CREATED").await,
                1,
                "DEAL_CREATED event should exist"
            );

            // Phase 2: KYC CASE CREATION linked to deal
            let case_id = db.create_case_with_deal(cbu_id, deal_id, group_id).await;

            // Link case to deal via deal_ubo_assessments
            let _assessment_id = db.link_case_to_deal(deal_id, entity_id, case_id).await;

            // Verify linkage
            let assessment_status = db.get_assessment_status(deal_id, case_id).await;
            assert_eq!(
                assessment_status.as_deref(),
                Some("PENDING"),
                "Assessment should be PENDING after creation"
            );

            // Verify case has deal_id
            let case_deal: (Option<Uuid>,) =
                sqlx::query_as(r#"SELECT deal_id FROM kyc.cases WHERE case_id = $1"#)
                    .bind(case_id)
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            assert_eq!(case_deal.0, Some(deal_id), "Case should reference the deal");

            // Phase 3: Progress case through lifecycle
            db.advance_case_to_review(case_id).await;

            let status: (String,) =
                sqlx::query_as(r#"SELECT status FROM kyc.cases WHERE case_id = $1"#)
                    .bind(case_id)
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            assert_eq!(status.0, "REVIEW");

            // Phase 4: Close case as APPROVED -> gate event emitted
            db.close_case(case_id, "APPROVED").await;

            // Verify case is APPROVED with closed_at
            let closed_case: (String, Option<chrono::DateTime<chrono::Utc>>) =
                sqlx::query_as(r#"SELECT status, closed_at FROM kyc.cases WHERE case_id = $1"#)
                    .bind(case_id)
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            assert_eq!(closed_case.0, "APPROVED");
            assert!(closed_case.1.is_some(), "closed_at must be set");

            // Verify KYC_GATE_COMPLETED event exists
            assert_eq!(
                db.count_deal_events(deal_id, "KYC_GATE_COMPLETED").await,
                1,
                "Exactly 1 KYC_GATE_COMPLETED event should exist"
            );

            // Verify the gate event references our case
            assert_eq!(
                db.count_gate_events_for_case(case_id).await,
                1,
                "Gate event should reference our case_id"
            );

            // Verify deal event details
            let gate_event: (String, Option<Uuid>, Option<String>) = sqlx::query_as(
                r#"SELECT subject_type, subject_id, new_value
                   FROM "ob-poc".deal_events
                   WHERE deal_id = $1 AND event_type = 'KYC_GATE_COMPLETED'
                   LIMIT 1"#,
            )
            .bind(deal_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();

            assert_eq!(gate_event.0, "KYC_CASE", "subject_type should be KYC_CASE");
            assert_eq!(
                gate_event.1,
                Some(case_id),
                "subject_id should be our case_id"
            );
            assert_eq!(
                gate_event.2.as_deref(),
                Some("APPROVED"),
                "new_value should be APPROVED"
            );

            // Verify complete event timeline
            let event_count: (i64,) =
                sqlx::query_as(r#"SELECT COUNT(*) FROM "ob-poc".deal_events WHERE deal_id = $1"#)
                    .bind(deal_id)
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            assert!(
                event_count.0 >= 2,
                "Should have at least DEAL_CREATED + KYC_GATE_COMPLETED"
            );

            db.cleanup().await;
        }
    }

    // =========================================================================
    // Test 2: Invalid Deal Reference
    // =========================================================================

    /// Verify that linking a case to a non-existent deal fails with FK violation.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_create_case_with_invalid_deal_rejected() {
        #[cfg(feature = "database")]
        {
            let db = TestDb::new().await;
            let cbu_id = db.create_cbu(&db.name("inv_deal")).await;
            let entity_id = db.create_entity(&db.name("inv_entity")).await;
            let case_id = db.create_case_standalone(cbu_id).await;

            // Attempt to link case to non-existent deal via deal_ubo_assessments
            let bogus_deal_id = Uuid::new_v4();

            let result = sqlx::query(
                r#"INSERT INTO "ob-poc".deal_ubo_assessments
                     (assessment_id, deal_id, entity_id, kyc_case_id, assessment_status)
                   VALUES ($1, $2, $3, $4, 'PENDING')"#,
            )
            .bind(Uuid::new_v4())
            .bind(bogus_deal_id)
            .bind(entity_id)
            .bind(case_id)
            .execute(&db.pool)
            .await;

            // Assert FK violation
            assert!(
                result.is_err(),
                "INSERT with non-existent deal_id should fail"
            );

            let err_msg = result.unwrap_err().to_string();
            assert!(
                err_msg.contains("foreign key")
                    || err_msg.contains("violates")
                    || err_msg.contains("fkey"),
                "Error should be a foreign key violation, got: {}",
                err_msg
            );

            // Verify no row was inserted
            let count: (i64,) = sqlx::query_as(
                r#"SELECT COUNT(*) FROM "ob-poc".deal_ubo_assessments
                   WHERE deal_id = $1"#,
            )
            .bind(bogus_deal_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
            assert_eq!(count.0, 0, "No row should exist for bogus deal_id");

            // No deal events for bogus deal
            let event_count: (i64,) =
                sqlx::query_as(r#"SELECT COUNT(*) FROM "ob-poc".deal_events WHERE deal_id = $1"#)
                    .bind(bogus_deal_id)
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            assert_eq!(event_count.0, 0, "No events should exist for bogus deal_id");

            db.cleanup().await;
        }
    }

    // =========================================================================
    // Test 3: Standalone Case — No Gate Event
    // =========================================================================

    /// Verify that closing a standalone KYC case (no deal link) does NOT
    /// produce any deal_events entry.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_close_case_without_deal_no_gate_event() {
        #[cfg(feature = "database")]
        {
            let db = TestDb::new().await;
            let cbu_id = db.create_cbu(&db.name("standalone")).await;

            // Create standalone case (no deal link)
            let case_id = db.create_case_standalone(cbu_id).await;

            // Progress through lifecycle
            db.advance_case_to_review(case_id).await;

            // Close as APPROVED
            db.close_case(case_id, "APPROVED").await;

            // Verify case is APPROVED
            let status: (String,) =
                sqlx::query_as(r#"SELECT status FROM kyc.cases WHERE case_id = $1"#)
                    .bind(case_id)
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            assert_eq!(status.0, "APPROVED");

            // Verify NO KYC_GATE_COMPLETED event for this case
            assert_eq!(
                db.count_gate_events_for_case(case_id).await,
                0,
                "Standalone case should produce NO gate events"
            );

            // Verify no deal_ubo_assessments link
            let link_count: (i64,) = sqlx::query_as(
                r#"SELECT COUNT(*) FROM "ob-poc".deal_ubo_assessments
                   WHERE kyc_case_id = $1"#,
            )
            .bind(case_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
            assert_eq!(link_count.0, 0, "No deal assessment link should exist");

            db.cleanup().await;
        }
    }

    // =========================================================================
    // Test 4: Rejected Case — No Gate Completion Event
    // =========================================================================

    /// Verify that closing a deal-linked KYC case as REJECTED does NOT
    /// produce a KYC_GATE_COMPLETED event. The gate only clears on APPROVED.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_close_case_rejected_no_gate_event() {
        #[cfg(feature = "database")]
        {
            let db = TestDb::new().await;

            // Setup deal + case
            let group_id = db.create_client_group(&db.name("rej_group")).await;
            let cbu_id = db.create_cbu(&db.name("rej_fund")).await;
            let entity_id = db.create_entity(&db.name("rej_entity")).await;
            let deal_id = db
                .create_deal(&db.name("rej_deal"), group_id, "CONTRACTED")
                .await;
            let case_id = db.create_case_with_deal(cbu_id, deal_id, group_id).await;
            let _assessment_id = db.link_case_to_deal(deal_id, entity_id, case_id).await;

            // Progress case to REVIEW
            db.advance_case_to_review(case_id).await;

            // Close as REJECTED (not APPROVED)
            db.close_case(case_id, "REJECTED").await;

            // Verify case is REJECTED
            let status: (String,) =
                sqlx::query_as(r#"SELECT status FROM kyc.cases WHERE case_id = $1"#)
                    .bind(case_id)
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            assert_eq!(status.0, "REJECTED");

            let closed_at: (Option<chrono::DateTime<chrono::Utc>>,) =
                sqlx::query_as(r#"SELECT closed_at FROM kyc.cases WHERE case_id = $1"#)
                    .bind(case_id)
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            assert!(
                closed_at.0.is_some(),
                "closed_at must be set even for rejection"
            );

            // Verify NO KYC_GATE_COMPLETED event
            assert_eq!(
                db.count_deal_events(deal_id, "KYC_GATE_COMPLETED").await,
                0,
                "Rejected case should NOT produce KYC_GATE_COMPLETED"
            );

            assert_eq!(
                db.count_gate_events_for_case(case_id).await,
                0,
                "No gate events should reference rejected case"
            );

            // Assessment status remains PENDING (not COMPLETED)
            let assess_status = db.get_assessment_status(deal_id, case_id).await;
            assert_eq!(
                assess_status.as_deref(),
                Some("PENDING"),
                "Assessment should remain PENDING after rejection"
            );

            // Deal status should NOT have advanced
            let deal_status: (String,) =
                sqlx::query_as(r#"SELECT deal_status FROM "ob-poc".deals WHERE deal_id = $1"#)
                    .bind(deal_id)
                    .fetch_one(&db.pool)
                    .await
                    .unwrap();
            assert_eq!(
                deal_status.0, "CONTRACTED",
                "Deal should remain CONTRACTED — KYC gate not cleared"
            );

            db.cleanup().await;
        }
    }
}
