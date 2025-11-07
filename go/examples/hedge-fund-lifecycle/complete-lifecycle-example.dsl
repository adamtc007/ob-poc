;;; ============================================================================
;;; HEDGE FUND INVESTOR LIFECYCLE - COMPLETE DSL EXAMPLE
;;; ============================================================================
;;;
;;; This example demonstrates the complete lifecycle of a hedge fund investor
;;; from initial opportunity identification through offboarding.
;;;
;;; Investor: Acme Capital Partners LP (Corporate Investor)
;;; Fund: Global Opportunities Hedge Fund (Class A USD)
;;; Initial Investment: $5,000,000 USD
;;; Timeline: Q1 2024 - Q4 2024
;;;
;;; Database Table: "hf-investor".hf_dsl_executions
;;; Each DSL operation below would be persisted in this table with:
;;;   - execution_id: UUID
;;;   - investor_id: UUID (once created)
;;;   - dsl_text: TEXT (the S-expression below)
;;;   - execution_status: 'COMPLETED'
;;;   - triggered_by: 'operations@fundadmin.com'
;;; ============================================================================

;;; ----------------------------------------------------------------------------
;;; STATE 1: OPPORTUNITY
;;; Initial lead captured in the system
;;; Triggered by: Marketing/Sales team
;;; ----------------------------------------------------------------------------

(investor.start-opportunity
  :legal-name "Acme Capital Partners LP"
  :type "CORPORATE"
  :domicile "US"
  :source "Institutional Investor Conference Q1 2024")

;;; Result:
;;; - investor_id: "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
;;; - status: OPPORTUNITY
;;; - investor_code: "INV-2024-001"
;;; - created_at: 2024-01-15T10:00:00Z


;;; ----------------------------------------------------------------------------
;;; STATE 2: PRECHECKS
;;; Investor expresses interest, indication of interest recorded
;;; Triggered by: Sales confirms investment appetite
;;; State Transition: OPPORTUNITY → PRECHECKS
;;; ----------------------------------------------------------------------------

(investor.record-indication
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :fund "f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c"
  :class "c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f"
  :ticket 5000000.00000000
  :currency "USD")

;;; Result:
;;; - indication_id: "i1j2k3l4-m5n6-4o7p-8q9r-0s1t2u3v4w5x"
;;; - status: PRECHECKS
;;; - ticket_size: $5,000,000
;;; - indicated_at: 2024-01-20T14:30:00Z
;;; Guard Condition Met: indication_recorded = true


;;; ----------------------------------------------------------------------------
;;; STATE 3: KYC_PENDING
;;; KYC process initiated, documents being collected
;;; Triggered by: Compliance team
;;; State Transition: PRECHECKS → KYC_PENDING
;;; ----------------------------------------------------------------------------

(kyc.begin
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :tier "STANDARD")

;;; Result:
;;; - kyc_profile_id: "k1y2c3p4-r5o6-4f7i-8l9e-0a1b2c3d4e5f"
;;; - status: KYC_PENDING
;;; - tier: STANDARD
;;; - initiated_at: 2024-01-22T09:00:00Z
;;; Guard Condition Met: initial_documents_submitted = true

;;; Collect Certificate of Incorporation
(kyc.collect-doc
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :doc-type "CERTIFICATE_OF_INCORPORATION"
  :subject "primary_entity"
  :file-path "/docs/kyc/acme-capital/cert-of-inc.pdf")

;;; Result:
;;; - document_id: "d1o2c3-4567-8901-2345-678901234567"
;;; - collected_at: 2024-01-25T11:15:00Z
;;; - expiry_date: NULL (no expiry)

;;; Collect Partnership Agreement
(kyc.collect-doc
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :doc-type "PARTNERSHIP_AGREEMENT"
  :subject "primary_entity"
  :file-path "/docs/kyc/acme-capital/partnership-agreement.pdf")

;;; Result:
;;; - document_id: "d2o2c3-4567-8901-2345-678901234568"
;;; - collected_at: 2024-01-25T11:20:00Z

;;; Collect Authorized Signatory List
(kyc.collect-doc
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :doc-type "AUTHORIZED_SIGNATORY_LIST"
  :subject "signatories"
  :file-path "/docs/kyc/acme-capital/auth-signatories.pdf")

;;; Result:
;;; - document_id: "d3o2c3-4567-8901-2345-678901234569"
;;; - collected_at: 2024-01-26T10:00:00Z

;;; Collect Managing Partner Passport
(kyc.collect-doc
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :doc-type "PASSPORT"
  :subject "managing_partner_john_smith"
  :file-path "/docs/kyc/acme-capital/john-smith-passport.pdf")

;;; Result:
;;; - document_id: "d4o2c3-4567-8901-2345-678901234570"
;;; - collected_at: 2024-01-26T10:30:00Z
;;; - expiry_date: 2029-06-15

;;; Perform AML/Sanctions Screening
(kyc.screen
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :provider "worldcheck")

;;; Result:
;;; - screening_id: "s1c2r3-4567-8901-2345-678901234571"
;;; - screening_result: "CLEAR"
;;; - screening_date: 2024-01-28T15:45:00Z
;;; - provider: worldcheck
;;; - matches_found: 0
;;; Guard Condition Met: screening_passed = true


;;; ----------------------------------------------------------------------------
;;; STATE 4: KYC_APPROVED
;;; KYC completed, investor approved for investment
;;; Triggered by: Head of Compliance
;;; State Transition: KYC_PENDING → KYC_APPROVED
;;; ----------------------------------------------------------------------------

(kyc.approve
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :risk "MEDIUM"
  :refresh-due "2025-01-28"
  :approved-by "Sarah Johnson, Head of Compliance"
  :comments "Standard institutional investor, all docs verified, no adverse findings")

;;; Result:
;;; - status: KYC_APPROVED
;;; - risk_rating: MEDIUM
;;; - approved_at: 2024-01-30T16:00:00Z
;;; - next_refresh_due: 2025-01-28
;;; Guard Conditions Met:
;;;   - documents_verified = true
;;;   - screening_passed = true
;;;   - risk_rating_assigned = true

;;; Set KYC Refresh Schedule
(kyc.refresh-schedule
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :frequency "ANNUAL"
  :next "2025-01-28")

;;; Result:
;;; - refresh_schedule_id: "r1f2r3-4567-8901-2345-678901234572"
;;; - frequency: ANNUAL
;;; - next_refresh: 2025-01-28

;;; Set Continuous Screening
(screen.continuous
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :frequency "DAILY")

;;; Result:
;;; - continuous_screening_id: "c1s2c3-4567-8901-2345-678901234573"
;;; - frequency: DAILY
;;; - enabled: true
;;; - last_screened: 2024-01-30T23:59:00Z

;;; Capture Tax Information
(tax.capture
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :fatca "NON_US_PERSON"
  :crs "ENTITY"
  :form "W8_BEN_E"
  :tin-type "EIN"
  :tin-value "XX-XXXXXXX")

;;; Result:
;;; - tax_profile_id: "t1a2x3-4567-8901-2345-678901234574"
;;; - fatca_status: NON_US_PERSON
;;; - crs_classification: ENTITY
;;; - form_type: W8_BEN_E
;;; - form_received_date: 2024-02-01
;;; - form_expiry_date: 2026-12-31

;;; Set Banking Instructions
(bank.set-instruction
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :currency "USD"
  :bank-name "JPMorgan Chase Bank N.A."
  :account-name "Acme Capital Partners LP"
  :swift "CHASUS33"
  :account-num "1234567890")

;;; Result:
;;; - bank_instruction_id: "b1n2k3-4567-8901-2345-678901234575"
;;; - currency: USD
;;; - status: ACTIVE
;;; - effective_from: 2024-02-01
;;; Guard Condition Met: banking_instructions_set = true


;;; ----------------------------------------------------------------------------
;;; STATE 5: SUB_PENDING_CASH
;;; Subscription requested, awaiting cash settlement
;;; Triggered by: Investor Relations / Operations
;;; State Transition: KYC_APPROVED → SUB_PENDING_CASH
;;; ----------------------------------------------------------------------------

(subscribe.request
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :fund "f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c"
  :class "c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f"
  :amount 5000000.00000000
  :currency "USD"
  :trade-date "2024-02-05"
  :value-date "2024-02-10")

;;; Result:
;;; - trade_id: "t1r2a3-4567-8901-2345-678901234576"
;;; - status: SUB_PENDING_CASH
;;; - trade_type: SUBSCRIPTION
;;; - subscription_amount: $5,000,000
;;; - trade_date: 2024-02-05
;;; - value_date: 2024-02-10
;;; - settlement_currency: USD
;;; Guard Conditions Met:
;;;   - valid_subscription_order = true
;;;   - minimum_investment_met = true (min: $1,000,000)
;;;   - banking_instructions_set = true


;;; ----------------------------------------------------------------------------
;;; STATE 6: FUNDED_PENDING_NAV
;;; Cash received, awaiting NAV strike for allocation
;;; Triggered by: Middle Office / Settlement team
;;; State Transition: SUB_PENDING_CASH → FUNDED_PENDING_NAV
;;; ----------------------------------------------------------------------------

(cash.confirm
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :trade "t1r2a3-4567-8901-2345-678901234576"
  :amount 5000000.00000000
  :value-date "2024-02-10"
  :bank-currency "USD"
  :reference "ACME-SUB-20240210-001")

;;; Result:
;;; - cash_confirmation_id: "c1a2s3-4567-8901-2345-678901234577"
;;; - status: FUNDED_PENDING_NAV
;;; - cash_received: $5,000,000
;;; - received_date: 2024-02-10T10:30:00Z
;;; - bank_reference: "ACME-SUB-20240210-001"
;;; Guard Condition Met: settlement_funds_received = true

;;; NAV Strike for Dealing Date
(deal.nav
  :fund "f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c"
  :class "c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f"
  :nav-date "2024-02-10"
  :nav 1250.75000000)

;;; Result:
;;; - nav_id: "n1a2v3-4567-8901-2345-678901234578"
;;; - fund: Global Opportunities Hedge Fund
;;; - class: A USD
;;; - nav_date: 2024-02-10
;;; - nav_per_share: $1,250.75
;;; - status: FINAL
;;; Guard Condition Met: nav_struck = true


;;; ----------------------------------------------------------------------------
;;; STATE 7: ISSUED → STATE 8: ACTIVE
;;; Units allocated to investor, position established
;;; Triggered by: Fund Accounting / Registry team
;;; State Transition: FUNDED_PENDING_NAV → ISSUED → ACTIVE
;;; ----------------------------------------------------------------------------

(subscribe.issue
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :trade "t1r2a3-4567-8901-2345-678901234576"
  :class "c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f"
  :series "s1e2r3-4567-8901-2345-678901234579"
  :nav-per-share 1250.75000000
  :units 3997.60000000)

;;; Result:
;;; - register_event_id: "r1e2g3-4567-8901-2345-678901234580"
;;; - status: ISSUED → ACTIVE (automatic transition)
;;; - event_type: ISSUE
;;; - units_allocated: 3,997.60 units
;;; - nav_per_share: $1,250.75
;;; - cost_basis: $5,000,000 / $1,250.75 = 3,997.60 units
;;; - allocation_date: 2024-02-12T09:00:00Z
;;; Guard Conditions Met:
;;;   - nav_struck = true
;;;   - units_allocated = true
;;;
;;; Register Lot Created:
;;;   - lot_id: "l1o2t3-4567-8901-2345-678901234581"
;;;   - investor_id: "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
;;;   - fund_id: "f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c"
;;;   - class_id: "c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f"
;;;   - series_id: "s1e2r3-4567-8901-2345-678901234579"
;;;   - units: 3,997.60
;;;   - total_cost: $5,000,000
;;;   - average_cost: $1,250.75/unit
;;;   - status: ACTIVE
;;;
;;; Investor Status: ACTIVE


;;; ----------------------------------------------------------------------------
;;; ACTIVE STATE: Ongoing Operations
;;; Investor holds active position, subject to ongoing monitoring
;;; Timeline: February 2024 - October 2024
;;; ----------------------------------------------------------------------------

;;; During the ACTIVE state, various operational activities occur:
;;;
;;; 1. Daily Continuous Screening (automated)
;;;    - Runs daily against worldcheck
;;;    - No hits detected during holding period
;;;
;;; 2. Monthly Reporting
;;;    - Position statements generated
;;;    - Performance reports distributed
;;;    - NAV updates provided
;;;
;;; 3. Quarterly Fund Valuation
;;;    - NAV updated monthly
;;;    - Position marked to market
;;;    - Unrealized P&L tracked
;;;
;;; Current Position as of 2024-10-30:
;;;   - Units Held: 3,997.60
;;;   - Current NAV: $1,382.45 per share
;;;   - Market Value: $5,526,789.20
;;;   - Unrealized Gain: $526,789.20 (+10.54%)


;;; ----------------------------------------------------------------------------
;;; STATE 9: REDEEM_PENDING
;;; Investor requests redemption
;;; Triggered by: Investor instruction via email
;;; State Transition: ACTIVE → REDEEM_PENDING
;;; ----------------------------------------------------------------------------

(redeem.request
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :class "c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f"
  :percentage 100.00000000
  :notice-date "2024-10-31"
  :value-date "2024-12-31")

;;; Result:
;;; - trade_id: "t2r2d3-4567-8901-2345-678901234582"
;;; - status: REDEEM_PENDING
;;; - trade_type: REDEMPTION
;;; - redemption_units: 3,997.60 (100% of holdings)
;;; - notice_date: 2024-10-31
;;; - value_date: 2024-12-31 (90-day notice met)
;;; - estimated_value: $5,526,789.20 (at current NAV)
;;; Guard Conditions Met:
;;;   - valid_redemption_notice = true
;;;   - notice_period_satisfied = true (90 days required, 61 days given)

;;; NOTE: Alternative redemption by units:
;;; (redeem.request
;;;   :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
;;;   :class "c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f"
;;;   :units 3997.60000000
;;;   :notice-date "2024-10-31"
;;;   :value-date "2024-12-31")

;;; Final NAV Strike for Redemption
(deal.nav
  :fund "f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c"
  :class "c1d2e3f4-a5b6-4c7d-8e9f-0a1b2c3d4e5f"
  :nav-date "2024-12-31"
  :nav 1402.30000000)

;;; Result:
;;; - nav_id: "n2a2v3-4567-8901-2345-678901234583"
;;; - nav_date: 2024-12-31
;;; - nav_per_share: $1,402.30
;;; - Final redemption value: 3,997.60 units × $1,402.30 = $5,604,828.48


;;; ----------------------------------------------------------------------------
;;; STATE 10: REDEEMED
;;; Redemption settled, cash paid to investor
;;; Triggered by: Treasury / Settlement team
;;; State Transition: REDEEM_PENDING → REDEEMED
;;; ----------------------------------------------------------------------------

(redeem.settle
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :trade "t2r2d3-4567-8901-2345-678901234582"
  :amount 5604828.48000000
  :settle-date "2025-01-05"
  :reference "ACME-RED-20250105-001")

;;; Result:
;;; - settlement_id: "s1e2t3-4567-8901-2345-678901234584"
;;; - status: REDEEMED
;;; - settlement_amount: $5,604,828.48
;;; - settlement_date: 2025-01-05
;;; - payment_reference: "ACME-RED-20250105-001"
;;; - realized_gain: $604,828.48
;;;
;;; Register Event Created:
;;;   - event_id: "r2e2g3-4567-8901-2345-678901234585"
;;;   - event_type: REDEEM
;;;   - delta_units: -3,997.60
;;;   - value_date: 2024-12-31
;;;   - nav_per_share: $1,402.30
;;;   - proceeds: $5,604,828.48
;;;
;;; Register Lot Updated:
;;;   - lot_id: "l1o2t3-4567-8901-2345-678901234581"
;;;   - units: 0.00 (fully redeemed)
;;;   - status: CLOSED
;;;
;;; Guard Conditions Met:
;;;   - units_redeemed = true
;;;   - cash_payment_made = true
;;;
;;; Investment Summary:
;;;   - Initial Investment: $5,000,000.00
;;;   - Redemption Proceeds: $5,604,828.48
;;;   - Total Gain: $604,828.48
;;;   - ROI: 12.10%
;;;   - Holding Period: 10 months (Feb 2024 - Dec 2024)


;;; ----------------------------------------------------------------------------
;;; STATE 11: OFFBOARDED
;;; Investor relationship closed, all documentation complete
;;; Triggered by: Client Services / Operations Manager
;;; State Transition: REDEEMED → OFFBOARDED
;;; ----------------------------------------------------------------------------

(offboard.close
  :investor "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :reason "Investor fully redeemed, relationship terminated per client request")

;;; Result:
;;; - offboarding_id: "o1f2f3-4567-8901-2345-678901234586"
;;; - status: OFFBOARDED (terminal state)
;;; - offboarded_at: 2025-01-10T10:00:00Z
;;; - offboarding_reason: "Full redemption, client request"
;;;
;;; Final State:
;;;   - investor_status: OFFBOARDED
;;;   - active_positions: 0
;;;   - total_register_events: 2 (ISSUE, REDEEM)
;;;   - lifecycle_states: 11 transitions recorded
;;;   - relationship_start: 2024-01-15
;;;   - relationship_end: 2025-01-10
;;;   - total_duration: 360 days
;;;
;;; Guard Condition Met:
;;;   - final_docs_complete = true
;;;
;;; Post-Offboarding:
;;;   - All documents archived
;;;   - Final tax reporting complete (1099 generated)
;;;   - Continuous screening disabled
;;;   - Banking instructions archived
;;;   - Audit trail preserved (immutable)
;;;   - Data retention per regulatory requirements (7 years)


;;; ============================================================================
;;; END OF LIFECYCLE EXAMPLE
;;; ============================================================================
;;;
;;; PERSISTENCE IN DATABASE:
;;;
;;; All DSL operations above are stored in:
;;;   Table: "hf-investor".hf_dsl_executions
;;;
;;; Sample row for the first operation:
;;;
;;; INSERT INTO "hf-investor".hf_dsl_executions (
;;;   execution_id,
;;;   investor_id,
;;;   dsl_text,
;;;   execution_status,
;;;   idempotency_key,
;;;   triggered_by,
;;;   execution_engine,
;;;   affected_entities,
;;;   execution_time_ms,
;;;   started_at,
;;;   completed_at,
;;;   created_at
;;; ) VALUES (
;;;   '11111111-2222-3333-4444-555555555555',
;;;   'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d',
;;;   '(investor.start-opportunity
;;;     :legal-name "Acme Capital Partners LP"
;;;     :type "CORPORATE"
;;;     :domicile "US"
;;;     :source "Institutional Investor Conference Q1 2024")',
;;;   'COMPLETED',
;;;   'sha256:abc123...',
;;;   'operations@fundadmin.com',
;;;   'hedge-fund-dsl-v1',
;;;   '{"investor_id": "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d", "investor_code": "INV-2024-001"}',
;;;   45,
;;;   '2024-01-15 10:00:00+00',
;;;   '2024-01-15 10:00:00.045+00',
;;;   '2024-01-15 10:00:00+00'
;;; );
;;;
;;; QUERY TO RETRIEVE LIFECYCLE:
;;;
;;; SELECT
;;;   execution_id,
;;;   dsl_text,
;;;   execution_status,
;;;   triggered_by,
;;;   completed_at
;;; FROM "hf-investor".hf_dsl_executions
;;; WHERE investor_id = 'a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d'
;;; ORDER BY created_at ASC;
;;;
;;; TOTAL OPERATIONS IN LIFECYCLE: 21
;;;   1. investor.start-opportunity       (OPPORTUNITY)
;;;   2. investor.record-indication       (PRECHECKS)
;;;   3. kyc.begin                        (KYC_PENDING)
;;;   4. kyc.collect-doc (×4)             (KYC_PENDING)
;;;   5. kyc.screen                       (KYC_PENDING)
;;;   6. kyc.approve                      (KYC_APPROVED)
;;;   7. kyc.refresh-schedule             (KYC_APPROVED)
;;;   8. screen.continuous                (KYC_APPROVED)
;;;   9. tax.capture                      (KYC_APPROVED)
;;;   10. bank.set-instruction            (KYC_APPROVED)
;;;   11. subscribe.request               (SUB_PENDING_CASH)
;;;   12. cash.confirm                    (FUNDED_PENDING_NAV)
;;;   13. deal.nav (1st)                  (FUNDED_PENDING_NAV)
;;;   14. subscribe.issue                 (ISSUED → ACTIVE)
;;;   15. redeem.request                  (REDEEM_PENDING)
;;;   16. deal.nav (2nd)                  (REDEEM_PENDING)
;;;   17. redeem.settle                   (REDEEMED)
;;;   18. offboard.close                  (OFFBOARDED)
;;;
;;; DSL VERBS DEMONSTRATED: 17 of 17 (100% coverage)
;;;
;;; ============================================================================
