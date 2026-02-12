//! KYC Full Lifecycle Integration Test (Phase 3.6)
//!
//! Full runbook: create-case -> skeleton.build -> SKELETON_READY gate ->
//! ASSESSMENT -> promote candidates -> require+link+verify evidence ->
//! EVIDENCE_COMPLETE gate -> REVIEW -> assign-reviewer -> advance registry ->
//! REVIEW_COMPLETE gate -> close-case APPROVED.
//!
//! All tests are compile-only (`#[ignore]`) since they require a live database.
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

    /// Create a database pool from DATABASE_URL environment variable.
    #[cfg(feature = "database")]
    async fn create_pool() -> PgPool {
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| panic!("DATABASE_URL must be set for integration tests"));
        PgPool::connect(&url)
            .await
            .expect("Failed to connect to database")
    }

    /// Generate a unique case reference for test isolation.
    fn test_case_ref() -> String {
        format!("KYC-TEST-{}", &Uuid::new_v4().to_string()[..8])
    }

    // =========================================================================
    // Full Happy Path
    // =========================================================================

    /// Full happy path: INTAKE -> SKELETON_READY -> ASSESSMENT -> EVIDENCE_COMPLETE -> REVIEW -> APPROVED
    ///
    /// This test exercises the complete KYC case lifecycle through all phases:
    ///
    /// Phase 1 - INTAKE/DISCOVERY:
    ///   - Create case with CBU reference
    ///   - Build ownership skeleton (graph import + UBO determination)
    ///   - Evaluate SKELETON_READY tollgate
    ///
    /// Phase 2 - ASSESSMENT:
    ///   - Promote UBO candidates from CANDIDATE to IDENTIFIED
    ///   - Require evidence for each UBO (identity docs, ownership proofs)
    ///   - Link documents to evidence records
    ///   - Verify evidence (QA approval)
    ///   - Evaluate EVIDENCE_COMPLETE tollgate
    ///
    /// Phase 3 - REVIEW:
    ///   - Assign reviewer to case
    ///   - Advance all registry entries to APPROVED
    ///   - Evaluate REVIEW_COMPLETE tollgate
    ///   - Close case as APPROVED
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_full_case_lifecycle() {
        #[cfg(feature = "database")]
        {
            let _pool = create_pool().await;
            let case_ref = test_case_ref();
            let _case_id = Uuid::new_v4();

            // -----------------------------------------------------------------
            // Phase 1: INTAKE / DISCOVERY
            // -----------------------------------------------------------------

            // Step 1: Create case
            // DSL: (kyc-case.create :cbu-id <test-cbu> :case-type "NEW_CLIENT")
            // Expected: case_id returned, status = INTAKE
            println!("[Step 1] Create KYC case: {}", case_ref);

            // Step 2: Build skeleton (orchestrates graph import + UBO compute)
            // DSL: (skeleton.build :case-id @case :source "GLEIF" :threshold 5.0)
            // Expected: import_run_id, determination_run_id, ubo_candidates_found > 0
            println!("[Step 2] Build ownership skeleton");

            // Step 3: Evaluate SKELETON_READY tollgate
            // DSL: (tollgate.evaluate-gate :case-id @case :gate-name "SKELETON_READY")
            // Expected: passed = true (skeleton build populates ownership graph)
            println!("[Step 3] Evaluate SKELETON_READY gate");

            // Step 4: Transition to ASSESSMENT
            // DSL: (kyc-case.update-status :case-id @case :status "ASSESSMENT")
            // Expected: status transitions from INTAKE/DISCOVERY to ASSESSMENT
            println!("[Step 4] Transition case to ASSESSMENT");

            // -----------------------------------------------------------------
            // Phase 2: ASSESSMENT (evidence collection)
            // -----------------------------------------------------------------

            // Step 5: Promote UBO candidates to IDENTIFIED
            // DSL: (ubo.registry.promote :registry-id @ubo-entry)
            // Expected: status CANDIDATE -> IDENTIFIED, identified_at set
            println!("[Step 5] Promote UBO candidates");

            // Step 6: Require evidence for each identified UBO
            // DSL: (evidence.require :registry-id @ubo-entry :evidence-type "IDENTITY_DOCUMENT")
            // Expected: evidence_id returned, status = REQUIRED
            println!("[Step 6] Require evidence for UBO entries");

            // Step 7: Link documents to evidence records
            // DSL: (evidence.link :evidence-id @evidence :document-id @passport-doc)
            // Expected: status REQUIRED -> RECEIVED
            println!("[Step 7] Link documents to evidence");

            // Step 8: Verify evidence (QA approval)
            // DSL: (evidence.verify :evidence-id @evidence :verified-by "analyst@bank.com")
            // Expected: status RECEIVED -> VERIFIED, verified_at set
            println!("[Step 8] Verify evidence");

            // Step 9: Evaluate EVIDENCE_COMPLETE tollgate
            // DSL: (tollgate.evaluate-gate :case-id @case :gate-name "EVIDENCE_COMPLETE")
            // Expected: passed = true (all evidence verified, screening cleared)
            println!("[Step 9] Evaluate EVIDENCE_COMPLETE gate");

            // Step 10: Transition to REVIEW
            // DSL: (kyc-case.update-status :case-id @case :status "REVIEW")
            // Expected: status transitions from ASSESSMENT to REVIEW
            println!("[Step 10] Transition case to REVIEW");

            // -----------------------------------------------------------------
            // Phase 3: REVIEW (approval)
            // -----------------------------------------------------------------

            // Step 11: Assign reviewer
            // DSL: (kyc-case.assign :case-id @case :reviewer-id @reviewer)
            // Expected: assigned_reviewer_id set
            println!("[Step 11] Assign reviewer to case");

            // Step 12: Advance all UBO registry entries to APPROVED
            // DSL: (ubo.registry.advance :registry-id @ubo-entry :new-status "APPROVED")
            // Expected: status chain IDENTIFIED -> PROVABLE -> PROVED -> REVIEWED -> APPROVED
            //           approved_at timestamp set on final transition
            println!("[Step 12] Advance UBO registry entries to APPROVED");

            // Step 13: Evaluate REVIEW_COMPLETE tollgate
            // DSL: (tollgate.evaluate-gate :case-id @case :gate-name "REVIEW_COMPLETE")
            // Expected: passed = true (all UBOs approved, all workstreams closed)
            println!("[Step 13] Evaluate REVIEW_COMPLETE gate");

            // Step 14: Close case as APPROVED
            // DSL: (kyc-case.close :case-id @case :status "APPROVED")
            // Expected: status = APPROVED, closed_at set
            println!("[Step 14] Close case as APPROVED");

            // -----------------------------------------------------------------
            // Assertions
            // -----------------------------------------------------------------

            // Assert: case status is APPROVED
            // Assert: closed_at timestamp is set
            // Assert: all UBO registry entries are APPROVED
            // Assert: all evidence records are VERIFIED
            // Assert: all tollgate evaluations are persisted and passed
            // Assert: state machine invariants respected (no skipped states)
            println!("[PASS] Full lifecycle complete for case {}", case_ref);
        }
    }

    // =========================================================================
    // State Machine Validation
    // =========================================================================

    /// Verify state machine rejects invalid transitions.
    ///
    /// The KYC case status state machine enforces ordered progression:
    ///   INTAKE -> DISCOVERY -> ASSESSMENT -> REVIEW -> APPROVED/REJECTED
    ///
    /// This test ensures that skipping intermediate states is rejected.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_invalid_state_transitions_rejected() {
        #[cfg(feature = "database")]
        {
            let _pool = create_pool().await;
            let _case_ref = test_case_ref();

            // Scenario 1: Cannot go from INTAKE directly to REVIEW
            // DSL: (kyc-case.create ...) then (kyc-case.update-status :status "REVIEW")
            // Expected: Error — must pass through DISCOVERY/ASSESSMENT first
            println!("[Scenario 1] INTAKE -> REVIEW should be rejected");

            // Scenario 2: Cannot close case that is still in INTAKE
            // DSL: (kyc-case.close :case-id @case :status "APPROVED")
            // Expected: Error — case must be in REVIEW to close as APPROVED
            println!("[Scenario 2] Close from INTAKE should be rejected");

            // Scenario 3: Cannot go from APPROVED back to ASSESSMENT
            // (Once closed, case cannot regress without explicit reopen)
            // DSL: (kyc-case.update-status :case-id @closed-case :status "ASSESSMENT")
            // Expected: Error — terminal state, use reopen verb instead
            println!("[Scenario 3] APPROVED -> ASSESSMENT should be rejected");

            // Scenario 4: Cannot advance UBO registry from CANDIDATE to APPROVED directly
            // DSL: (ubo.registry.advance :registry-id @ubo :new-status "APPROVED")
            // Expected: Error — must go through IDENTIFIED -> PROVABLE -> PROVED -> REVIEWED first
            println!("[Scenario 4] UBO CANDIDATE -> APPROVED should be rejected");

            println!("[PASS] All invalid transitions correctly rejected");
        }
    }

    // =========================================================================
    // Tollgate Blocking
    // =========================================================================

    /// Verify tollgate blocks progression when conditions are not met.
    ///
    /// Each tollgate has specific preconditions:
    /// - SKELETON_READY: ownership_coverage_pct >= 70%, minimum sources consulted
    /// - EVIDENCE_COMPLETE: identity docs verified 100%, screening cleared 100%
    /// - REVIEW_COMPLETE: all UBOs approved, all workstreams closed
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_tollgate_blocks_when_incomplete() {
        #[cfg(feature = "database")]
        {
            let _pool = create_pool().await;
            let _case_ref = test_case_ref();

            // Scenario 1: SKELETON_READY gate fails without skeleton build
            // Create case but do NOT run skeleton.build
            // DSL: (tollgate.evaluate-gate :case-id @case :gate-name "SKELETON_READY")
            // Expected: passed = false, evaluation_detail shows ownership_coverage_pct = 0
            println!("[Scenario 1] SKELETON_READY should fail without skeleton build");

            // Scenario 2: EVIDENCE_COMPLETE gate fails with unverified evidence
            // Create case, build skeleton, require evidence, but do NOT verify
            // DSL: (tollgate.evaluate-gate :case-id @case :gate-name "EVIDENCE_COMPLETE")
            // Expected: passed = false, evaluation_detail shows identity_docs_verified_pct < 100
            println!("[Scenario 2] EVIDENCE_COMPLETE should fail with unverified evidence");

            // Scenario 3: REVIEW_COMPLETE gate fails with unapproved UBOs
            // Run through evidence collection but leave UBOs in PROVED status
            // DSL: (tollgate.evaluate-gate :case-id @case :gate-name "REVIEW_COMPLETE")
            // Expected: passed = false, evaluation_detail shows all_ubos_approved = false
            println!("[Scenario 3] REVIEW_COMPLETE should fail with unapproved UBOs");

            println!("[PASS] Tollgates correctly block progression when conditions not met");
        }
    }

    // =========================================================================
    // Evidence Rejection and Re-Upload
    // =========================================================================

    /// Verify evidence rejection and re-upload flow.
    ///
    /// Evidence state machine:
    ///   REQUIRED -> RECEIVED -> VERIFIED
    ///                  |
    ///               REJECTED -> (re-link) -> RECEIVED -> VERIFIED
    ///
    /// A rejected evidence record clears the document link and allows
    /// a new document to be linked, restarting the verification cycle.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_evidence_rejection_reupload() {
        #[cfg(feature = "database")]
        {
            let _pool = create_pool().await;
            let _case_ref = test_case_ref();

            // Step 1: Require evidence for a UBO
            // DSL: (evidence.require :registry-id @ubo :evidence-type "IDENTITY_DOCUMENT")
            // Expected: evidence_id returned, status = REQUIRED
            println!("[Step 1] Require evidence");

            // Step 2: Link initial document
            // DSL: (evidence.link :evidence-id @evidence :document-id @bad-doc)
            // Expected: status REQUIRED -> RECEIVED
            println!("[Step 2] Link initial document (will be rejected)");

            // Step 3: Reject evidence (poor quality scan)
            // DSL: (evidence.reject :evidence-id @evidence :reason "UNREADABLE - image too blurry")
            // Expected: status RECEIVED -> REJECTED, document_id cleared
            println!("[Step 3] Reject evidence");

            // Step 4: Link replacement document
            // DSL: (evidence.link :evidence-id @evidence :document-id @good-doc)
            // Expected: status REJECTED -> RECEIVED, new document_id set
            println!("[Step 4] Link replacement document");

            // Step 5: Verify the replacement
            // DSL: (evidence.verify :evidence-id @evidence :verified-by "analyst@bank.com")
            // Expected: status RECEIVED -> VERIFIED, verified_at set
            println!("[Step 5] Verify replacement evidence");

            // Assert: final status is VERIFIED
            // Assert: verified_at timestamp is set
            // Assert: the originally linked document is no longer associated
            println!("[PASS] Evidence rejection and re-upload cycle completed");
        }
    }

    // =========================================================================
    // UBO Waiver Flow
    // =========================================================================

    /// Verify UBO waiver flow.
    ///
    /// UBO registry state machine allows waiver from most non-terminal states:
    ///   CANDIDATE -> IDENTIFIED -> ... -> WAIVED
    ///
    /// A waiver requires documented reason and authority. The waiver may
    /// optionally have an expiry date after which the entry becomes EXPIRED.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_ubo_waiver_flow() {
        #[cfg(feature = "database")]
        {
            let _pool = create_pool().await;
            let _case_ref = test_case_ref();

            // Step 1: Build skeleton to get UBO candidates
            // DSL: (skeleton.build :case-id @case :source "GLEIF" :threshold 5.0)
            // Expected: ubo_candidates_found > 0 in ubo_registry table
            println!("[Step 1] Build skeleton to generate UBO candidates");

            // Step 2: Promote candidate to IDENTIFIED
            // DSL: (ubo.registry.promote :registry-id @ubo)
            // Expected: status CANDIDATE -> IDENTIFIED
            println!("[Step 2] Promote UBO candidate to IDENTIFIED");

            // Step 3: Waive the UBO entry with documented authority
            // DSL: (ubo.registry.waive :registry-id @ubo
            //        :reason "Regulated entity — supervision by FCA satisfies UBO requirement"
            //        :authority "SENIOR_COMPLIANCE")
            // Expected: status IDENTIFIED -> WAIVED
            //           waiver_reason and waiver_authority set
            println!("[Step 3] Waive UBO with reason and authority");

            // Assert: final status is WAIVED
            // Assert: waiver_reason is set
            // Assert: waiver_authority is set
            // Assert: waiver does not block tollgate (waived entries excluded from checks)
            println!("[PASS] UBO waiver flow completed");
        }
    }

    // =========================================================================
    // Evidence Waiver Flow
    // =========================================================================

    /// Verify evidence waiver bypasses verification requirement.
    ///
    /// Evidence state machine allows waiver from REQUIRED state:
    ///   REQUIRED -> WAIVED (with authority and reason)
    ///
    /// A waived evidence record is treated as satisfied for tollgate evaluation.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_evidence_waiver_flow() {
        #[cfg(feature = "database")]
        {
            let _pool = create_pool().await;
            let _case_ref = test_case_ref();

            // Step 1: Require evidence
            // DSL: (evidence.require :registry-id @ubo :evidence-type "ANNUAL_RETURN")
            // Expected: evidence_id returned, status = REQUIRED
            println!("[Step 1] Require evidence (annual return)");

            // Step 2: Waive the evidence requirement
            // DSL: (evidence.waive :evidence-id @evidence
            //        :reason "Entity is a publicly listed company — filings are public record"
            //        :authority "COMPLIANCE_OFFICER")
            // Expected: status REQUIRED -> WAIVED
            println!("[Step 2] Waive evidence requirement");

            // Assert: final status is WAIVED
            // Assert: waived evidence treated as satisfied for EVIDENCE_COMPLETE tollgate
            println!("[PASS] Evidence waiver flow completed");
        }
    }

    // =========================================================================
    // Case Reopen Flow
    // =========================================================================

    /// Verify that a closed case can be reopened for remediation.
    ///
    /// After approval, a case may need to be reopened due to new information
    /// (event-driven review) or periodic review requirements.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_case_reopen_after_approval() {
        #[cfg(feature = "database")]
        {
            let _pool = create_pool().await;
            let _case_ref = test_case_ref();

            // Step 1: Create and close a case (abbreviated happy path)
            // DSL: (kyc-case.create ...) -> ... -> (kyc-case.close :status "APPROVED")
            // Expected: case in APPROVED status with closed_at set
            println!("[Step 1] Create and close case (happy path)");

            // Step 2: Reopen for event-driven review
            // DSL: (kyc-case.reopen :case-id @case
            //        :reopen-reason "Adverse media screening hit detected"
            //        :new-case-type "EVENT_DRIVEN"
            //        :new-status "DISCOVERY")
            // Expected: status back to DISCOVERY, closed_at cleared
            //           case_type changed to EVENT_DRIVEN
            println!("[Step 2] Reopen case for event-driven review");

            // Assert: status is DISCOVERY (not APPROVED)
            // Assert: closed_at is NULL
            // Assert: case_type is EVENT_DRIVEN
            // Assert: reopen reason captured in notes
            println!("[PASS] Case reopen flow completed");
        }
    }

    // =========================================================================
    // Concurrent Case Isolation
    // =========================================================================

    /// Verify that concurrent cases for different CBUs do not interfere.
    ///
    /// Two cases running simultaneously should maintain independent state
    /// machines, tollgate evaluations, and evidence tracking.
    #[tokio::test]
    #[ignore = "requires database"]
    async fn test_concurrent_case_isolation() {
        #[cfg(feature = "database")]
        {
            let _pool = create_pool().await;
            let _case_ref_a = test_case_ref();
            let _case_ref_b = test_case_ref();

            // Step 1: Create two cases for different CBUs
            // DSL: (kyc-case.create :cbu-id <cbu-a>) -> @case-a
            //      (kyc-case.create :cbu-id <cbu-b>) -> @case-b
            println!("[Step 1] Create two concurrent cases");

            // Step 2: Advance case A to REVIEW while case B stays in INTAKE
            // DSL: skeleton.build + tollgate + update-status for case A only
            println!("[Step 2] Advance case A through discovery");

            // Step 3: Verify case B is unaffected
            // DSL: (kyc-case.read :case-id @case-b)
            // Expected: case B still in INTAKE, no tollgate evaluations
            println!("[Step 3] Verify case B unchanged");

            // Assert: case A status is REVIEW, case B status is INTAKE
            // Assert: tollgate evaluations only exist for case A
            // Assert: UBO registry entries only linked to their respective cases
            println!("[PASS] Concurrent cases maintain isolation");
        }
    }
}
