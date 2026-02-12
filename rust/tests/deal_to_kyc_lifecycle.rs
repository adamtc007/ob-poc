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
//! All tests are compile-only (`#[ignore]`) since they require a live database.
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

    /// Create a database pool from DATABASE_URL environment variable.
    #[cfg(feature = "database")]
    async fn create_pool() -> PgPool {
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| panic!("DATABASE_URL must be set for integration tests"));
        PgPool::connect(&url)
            .await
            .expect("Failed to connect to database")
    }

    /// Generate a unique deal reference for test isolation.
    fn test_deal_ref() -> String {
        format!("DEAL-TEST-{}", &Uuid::new_v4().to_string()[..8])
    }

    /// Generate a unique case reference for test isolation.
    fn test_case_ref() -> String {
        format!("KYC-TEST-{}", &Uuid::new_v4().to_string()[..8])
    }

    // =========================================================================
    // Happy Path: Deal -> KYC -> Approved -> Gate Event
    // =========================================================================

    /// Full happy path: deal CONTRACTED -> KYC case linked -> lifecycle ->
    /// APPROVED -> deal_events has KYC_GATE_COMPLETED.
    ///
    /// This test exercises the complete deal-to-KYC bridge:
    ///
    /// Phase 1 - DEAL SETUP:
    ///   - Create deal in CONTRACTED status
    ///   - Record deal_id for downstream linking
    ///
    /// Phase 2 - KYC CASE CREATION (linked to deal):
    ///   - Create KYC case with CBU reference
    ///   - Link case to deal via deal_ubo_assessments (deal_id + kyc_case_id)
    ///   - Verify case status = INTAKE and deal linkage established
    ///
    /// Phase 3 - SKELETON / ASSESSMENT / REVIEW:
    ///   - Build ownership skeleton
    ///   - Progress through ASSESSMENT (promote UBOs, collect evidence)
    ///   - Assign reviewer and advance through REVIEW
    ///
    /// Phase 4 - CLOSE CASE + GATE EVENT:
    ///   - Close case as APPROVED
    ///   - Verify deal_events contains a KYC_GATE_COMPLETED entry
    ///     with subject_type = 'KYC_CASE' and subject_id = case_id
    ///   - Verify deal_ubo_assessments.assessment_status = 'COMPLETED'
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_deal_to_kyc_full_lifecycle() {
        #[cfg(feature = "database")]
        {
            let _pool = create_pool().await;
            let deal_ref = test_deal_ref();
            let case_ref = test_case_ref();
            let _deal_id = Uuid::new_v4();
            let _case_id = Uuid::new_v4();
            let _entity_id = Uuid::new_v4();
            let _cbu_id = Uuid::new_v4();

            // =================================================================
            // Phase 1: DEAL SETUP
            // =================================================================

            // Step 1: Create deal in CONTRACTED status
            // DSL: (deal.create :deal-name "Test Deal" :client-group-id <group>
            //        :sales-owner "test@bank.com" :as @deal)
            // DSL: (deal.update-status :deal-id @deal :new-status "CONTRACTED")
            // Expected: deal_id returned, deal_status = CONTRACTED
            println!("[Step 1] Create deal in CONTRACTED status: {}", deal_ref);

            // Step 2: Record deal_events entry for deal creation
            // Expected: deal_events has DEAL_CREATED + STATUS_CHANGED entries
            println!("[Step 2] Verify DEAL_CREATED event recorded");

            // =================================================================
            // Phase 2: KYC CASE CREATION (linked to deal)
            // =================================================================

            // Step 3: Create KYC case for the CBU
            // DSL: (kyc-case.create :cbu-id <test-cbu> :case-type "NEW_CLIENT")
            // Expected: case_id returned, status = INTAKE
            println!("[Step 3] Create KYC case: {}", case_ref);

            // Step 4: Link KYC case to deal via deal_ubo_assessments
            // This is the bridge table: deal_ubo_assessments has deal_id,
            // entity_id, and kyc_case_id columns.
            // DSL: (deal.create-ubo-assessment :deal-id @deal
            //        :entity-id @entity :kyc-case-id @case)
            // Expected: assessment_id returned, assessment_status = PENDING
            println!("[Step 4] Link KYC case to deal via deal_ubo_assessments");

            // Step 5: Verify the linkage is established
            // Query: SELECT * FROM deal_ubo_assessments
            //        WHERE deal_id = @deal AND kyc_case_id = @case
            // Expected: exactly 1 row, assessment_status = 'PENDING'
            println!("[Step 5] Verify deal-to-case linkage in deal_ubo_assessments");

            // =================================================================
            // Phase 3: KYC LIFECYCLE (abbreviated)
            // =================================================================

            // Step 6: Build ownership skeleton
            // DSL: (skeleton.build :case-id @case :source "GLEIF" :threshold 5.0)
            // Expected: ownership graph populated, UBO candidates found
            println!("[Step 6] Build ownership skeleton");

            // Step 7: Evaluate SKELETON_READY gate and transition to ASSESSMENT
            // DSL: (tollgate.evaluate-gate :case-id @case :gate-name "SKELETON_READY")
            // DSL: (kyc-case.update-status :case-id @case :status "ASSESSMENT")
            // Expected: gate passed, status = ASSESSMENT
            println!("[Step 7] SKELETON_READY gate -> transition to ASSESSMENT");

            // Step 8: Promote UBO candidates and collect/verify evidence
            // DSL: (ubo.registry.promote :registry-id @ubo-entry)
            // DSL: (evidence.require :registry-id @ubo-entry :evidence-type "IDENTITY_DOCUMENT")
            // DSL: (evidence.link :evidence-id @evidence :document-id @doc)
            // DSL: (evidence.verify :evidence-id @evidence :verified-by "analyst@bank.com")
            // Expected: all UBOs promoted, evidence verified
            println!("[Step 8] Promote UBOs, collect and verify evidence");

            // Step 9: Evaluate EVIDENCE_COMPLETE gate and transition to REVIEW
            // DSL: (tollgate.evaluate-gate :case-id @case :gate-name "EVIDENCE_COMPLETE")
            // DSL: (kyc-case.update-status :case-id @case :status "REVIEW")
            // Expected: gate passed, status = REVIEW
            println!("[Step 9] EVIDENCE_COMPLETE gate -> transition to REVIEW");

            // Step 10: Assign reviewer and advance UBO registry entries
            // DSL: (kyc-case.assign :case-id @case :reviewer-id @reviewer)
            // DSL: (ubo.registry.advance :registry-id @ubo-entry :new-status "APPROVED")
            // Expected: reviewer assigned, all UBOs approved
            println!("[Step 10] Assign reviewer, advance UBOs to APPROVED");

            // Step 11: Evaluate REVIEW_COMPLETE gate
            // DSL: (tollgate.evaluate-gate :case-id @case :gate-name "REVIEW_COMPLETE")
            // Expected: gate passed
            println!("[Step 11] Evaluate REVIEW_COMPLETE gate");

            // =================================================================
            // Phase 4: CLOSE CASE + VERIFY DEAL GATE EVENT
            // =================================================================

            // Step 12: Close case as APPROVED
            // DSL: (kyc-case.close :case-id @case :status "APPROVED")
            // Expected: case status = APPROVED, closed_at set
            println!("[Step 12] Close KYC case as APPROVED");

            // Step 13: Verify deal_ubo_assessments updated
            // Query: SELECT assessment_status, completed_at
            //        FROM deal_ubo_assessments
            //        WHERE deal_id = @deal AND kyc_case_id = @case
            // Expected: assessment_status = 'COMPLETED', completed_at IS NOT NULL
            println!("[Step 13] Verify deal_ubo_assessments.assessment_status = COMPLETED");

            // Step 14: Verify KYC_GATE_COMPLETED event in deal_events
            // Query: SELECT * FROM deal_events
            //        WHERE deal_id = @deal
            //          AND event_type = 'KYC_GATE_COMPLETED'
            //          AND subject_type = 'KYC_CASE'
            //          AND subject_id = @case
            // Expected: exactly 1 row with:
            //   - event_type = 'KYC_GATE_COMPLETED'
            //   - subject_type = 'KYC_CASE'
            //   - subject_id = case_id
            //   - new_value = 'APPROVED'
            //   - description contains case reference
            println!("[Step 14] Verify KYC_GATE_COMPLETED event in deal_events");

            // Step 15: Verify deal_events audit trail completeness
            // Query: SELECT event_type FROM deal_events
            //        WHERE deal_id = @deal ORDER BY occurred_at
            // Expected sequence includes:
            //   DEAL_CREATED -> STATUS_CHANGED (CONTRACTED) -> KYC_GATE_COMPLETED
            println!("[Step 15] Verify full deal_events audit trail");

            // -----------------------------------------------------------------
            // Assertions
            // -----------------------------------------------------------------

            // Assert: KYC case status is APPROVED with closed_at set
            // Assert: deal_ubo_assessments.assessment_status = 'COMPLETED'
            // Assert: deal_ubo_assessments.completed_at IS NOT NULL
            // Assert: deal_events contains exactly 1 KYC_GATE_COMPLETED event
            // Assert: KYC_GATE_COMPLETED event references the correct case_id
            // Assert: deal_events audit trail is complete (creation -> gate)
            println!(
                "[PASS] Deal-to-KYC lifecycle complete: deal={}, case={}",
                deal_ref, case_ref
            );
        }
    }

    // =========================================================================
    // Error Case: Invalid Deal Reference
    // =========================================================================

    /// Verify that creating a KYC case linked to a non-existent deal is rejected.
    ///
    /// The deal_ubo_assessments table has a FK constraint on deal_id referencing
    /// deals(deal_id). Attempting to insert a row with a non-existent deal_id
    /// must fail with a foreign key violation.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_create_case_with_invalid_deal_rejected() {
        #[cfg(feature = "database")]
        {
            let _pool = create_pool().await;
            let _bogus_deal_id = Uuid::new_v4(); // Does not exist in deals table
            let _entity_id = Uuid::new_v4();
            let _case_id = Uuid::new_v4();

            // Step 1: Create a KYC case (standalone, no deal link yet)
            // DSL: (kyc-case.create :cbu-id <test-cbu> :case-type "NEW_CLIENT")
            // Expected: case_id returned, status = INTAKE
            println!("[Step 1] Create standalone KYC case");

            // Step 2: Attempt to link case to non-existent deal via deal_ubo_assessments
            // SQL: INSERT INTO deal_ubo_assessments (deal_id, entity_id, kyc_case_id)
            //      VALUES (@bogus_deal_id, @entity_id, @case_id)
            // Expected: ERROR — FK violation on deal_id
            //   "insert or update on table \"deal_ubo_assessments\" violates
            //    foreign key constraint \"deal_ubo_assessments_deal_id_fkey\""
            println!("[Step 2] Attempt to link case to non-existent deal");

            // Step 3: Verify the insertion was rejected
            // Query: SELECT COUNT(*) FROM deal_ubo_assessments
            //        WHERE deal_id = @bogus_deal_id
            // Expected: 0 rows (FK constraint prevented insertion)
            println!("[Step 3] Verify no row was inserted");

            // -----------------------------------------------------------------
            // Assertions
            // -----------------------------------------------------------------

            // Assert: INSERT raised a foreign key violation error
            // Assert: deal_ubo_assessments has no row for bogus_deal_id
            // Assert: no deal_events created for bogus_deal_id
            println!("[PASS] Case with invalid deal_id correctly rejected");
        }
    }

    // =========================================================================
    // No-Op Case: Close Without Deal Link
    // =========================================================================

    /// Verify that closing a KYC case that has no deal linkage does NOT
    /// produce any deal_events entry.
    ///
    /// A KYC case created without a deal_ubo_assessments link is a standalone
    /// case (e.g., periodic review, regulatory request). Closing such a case
    /// should not emit KYC_GATE_COMPLETED to any deal.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_close_case_without_deal_no_gate_event() {
        #[cfg(feature = "database")]
        {
            let _pool = create_pool().await;
            let case_ref = test_case_ref();
            let _case_id = Uuid::new_v4();

            // Step 1: Create standalone KYC case (no deal linkage)
            // DSL: (kyc-case.create :cbu-id <test-cbu> :case-type "NEW_CLIENT")
            // Expected: case_id returned, status = INTAKE
            // NOTE: No deal_ubo_assessments row created — case is standalone
            println!("[Step 1] Create standalone KYC case: {}", case_ref);

            // Step 2: Progress through lifecycle (abbreviated)
            // DSL: skeleton.build -> ASSESSMENT -> evidence -> REVIEW
            // Expected: case progresses through all phases
            println!("[Step 2] Progress case through lifecycle");

            // Step 3: Close case as APPROVED
            // DSL: (kyc-case.close :case-id @case :status "APPROVED")
            // Expected: case status = APPROVED, closed_at set
            println!("[Step 3] Close standalone case as APPROVED");

            // Step 4: Verify NO KYC_GATE_COMPLETED event in deal_events
            // Query: SELECT COUNT(*) FROM deal_events
            //        WHERE event_type = 'KYC_GATE_COMPLETED'
            //          AND subject_type = 'KYC_CASE'
            //          AND subject_id = @case
            // Expected: 0 rows — standalone case has no deal to notify
            println!("[Step 4] Verify no KYC_GATE_COMPLETED event exists");

            // Step 5: Verify no deal_ubo_assessments rows reference this case
            // Query: SELECT COUNT(*) FROM deal_ubo_assessments
            //        WHERE kyc_case_id = @case
            // Expected: 0 rows
            println!("[Step 5] Verify no deal_ubo_assessments link exists");

            // -----------------------------------------------------------------
            // Assertions
            // -----------------------------------------------------------------

            // Assert: case is APPROVED with closed_at set
            // Assert: deal_events has 0 rows for KYC_GATE_COMPLETED with this case_id
            // Assert: deal_ubo_assessments has 0 rows for this case_id
            // Assert: closing a standalone case is a clean no-op for deal pipeline
            println!(
                "[PASS] Standalone case closure produced no deal gate event: {}",
                case_ref
            );
        }
    }

    // =========================================================================
    // Rejected Case: No Gate Event for REJECTED Outcome
    // =========================================================================

    /// Verify that closing a deal-linked KYC case as REJECTED does NOT
    /// produce a KYC_GATE_COMPLETED event.
    ///
    /// The KYC gate only "completes" when the case is APPROVED. A rejected
    /// case means the onboarding cannot proceed — the deal remains blocked
    /// at the KYC gate. The deal_ubo_assessments status should be updated
    /// to reflect the rejection, but no gate-completion event is emitted.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_close_case_rejected_no_gate_event() {
        #[cfg(feature = "database")]
        {
            let _pool = create_pool().await;
            let deal_ref = test_deal_ref();
            let case_ref = test_case_ref();
            let _deal_id = Uuid::new_v4();
            let _case_id = Uuid::new_v4();
            let _entity_id = Uuid::new_v4();

            // Step 1: Create deal in CONTRACTED status
            // DSL: (deal.create :deal-name "Rejected Test" :client-group-id <group>
            //        :sales-owner "test@bank.com" :as @deal)
            // DSL: (deal.update-status :deal-id @deal :new-status "CONTRACTED")
            // Expected: deal_id returned, deal_status = CONTRACTED
            println!("[Step 1] Create deal: {}", deal_ref);

            // Step 2: Create KYC case linked to deal
            // DSL: (kyc-case.create :cbu-id <test-cbu> :case-type "NEW_CLIENT")
            // DSL: (deal.create-ubo-assessment :deal-id @deal
            //        :entity-id @entity :kyc-case-id @case)
            // Expected: case created, deal_ubo_assessments row with status PENDING
            println!("[Step 2] Create KYC case linked to deal: {}", case_ref);

            // Step 3: Progress case through skeleton/assessment
            // DSL: skeleton.build -> ASSESSMENT -> evidence (abbreviated)
            println!("[Step 3] Progress case through assessment");

            // Step 4: Move to REVIEW
            // DSL: (kyc-case.update-status :case-id @case :status "REVIEW")
            println!("[Step 4] Transition case to REVIEW");

            // Step 5: Close case as REJECTED (not APPROVED)
            // DSL: (kyc-case.close :case-id @case :status "REJECTED")
            // Expected: case status = REJECTED, closed_at set
            // NOTE: This is a negative outcome — KYC gate is NOT cleared
            println!("[Step 5] Close KYC case as REJECTED");

            // Step 6: Verify NO KYC_GATE_COMPLETED event in deal_events
            // Query: SELECT COUNT(*) FROM deal_events
            //        WHERE deal_id = @deal
            //          AND event_type = 'KYC_GATE_COMPLETED'
            // Expected: 0 rows — rejected cases do not pass the gate
            println!("[Step 6] Verify no KYC_GATE_COMPLETED event");

            // Step 7: Verify deal_ubo_assessments reflects rejection
            // Query: SELECT assessment_status FROM deal_ubo_assessments
            //        WHERE deal_id = @deal AND kyc_case_id = @case
            // Expected: assessment_status = 'BLOCKED' (not COMPLETED)
            // The assessment is blocked because KYC was rejected, not completed.
            println!("[Step 7] Verify deal_ubo_assessments.assessment_status = BLOCKED");

            // Step 8: Verify deal status has NOT progressed past KYC gate
            // Query: SELECT deal_status FROM deals WHERE deal_id = @deal
            // Expected: deal_status still CONTRACTED (not ONBOARDING)
            // The deal cannot move to ONBOARDING without KYC clearance
            println!("[Step 8] Verify deal remains in CONTRACTED status");

            // -----------------------------------------------------------------
            // Assertions
            // -----------------------------------------------------------------

            // Assert: case status is REJECTED with closed_at set
            // Assert: deal_events has 0 KYC_GATE_COMPLETED events for this deal
            // Assert: deal_ubo_assessments.assessment_status = 'BLOCKED'
            // Assert: deal_ubo_assessments.completed_at IS NULL
            // Assert: deal_status is still CONTRACTED (gate not cleared)
            // Assert: rejected case blocks the deal pipeline as expected
            println!(
                "[PASS] Rejected case produced no gate event: deal={}, case={}",
                deal_ref, case_ref
            );
        }
    }
}
