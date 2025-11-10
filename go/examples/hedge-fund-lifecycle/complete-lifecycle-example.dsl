;;; ============================================================================
;;; HEDGE FUND INVESTOR LIFECYCLE - COMPLETE DSL EXAMPLE (v3.0)
;;; ============================================================================
;;;
;;; This example demonstrates the complete lifecycle of a hedge fund investor
;;; from initial opportunity identification through offboarding using v3.0 syntax.
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
  :source "Institutional Investor Conference Q1 2024"
  :created-at "2024-01-15T10:00:00Z")

;;; Result:
;;; - investor_id: "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
;;; - status: OPPORTUNITY
;;; - investor_code: "INV-2024-001"

;;; ----------------------------------------------------------------------------
;;; STATE 2: PRECHECKS
;;; Investor expresses interest, indication of interest recorded
;;; Triggered by: Sales confirms investment appetite
;;; State Transition: OPPORTUNITY → PRECHECKS
;;; ----------------------------------------------------------------------------

(investor.record-indication
  :investor-id "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :fund-id "f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c"
  :fund-name "Global Opportunities Hedge Fund"
  :share-class "Class A USD"
  :indication-amount 5000000.00
  :currency "USD"
  :expected-timing "Q2 2024"
  :recorded-at "2024-02-01T14:30:00Z")

;;; Result:
;;; - status: OPPORTUNITY → PRECHECKS
;;; - indication_of_interest_id: "i1o2i3-a4b5c6-d7e8f9"

;;; ----------------------------------------------------------------------------
;;; STATE 3: SUBSCRIPTION DOCS
;;; Subscription documentation preparation and delivery
;;; State Transition: PRECHECKS → SUBSCRIPTION_DOCS
;;; ----------------------------------------------------------------------------

(investor.prepare-subscription-docs
  :investor-id "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :fund-id "f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c"
  :documents [
    "SUBSCRIPTION_AGREEMENT",
    "FUND_PPM",
    "RISK_DISCLOSURE",
    "INVESTOR_QUESTIONNAIRE",
    "ANTI_MONEY_LAUNDERING_FORMS"
  ]
  :delivery-method "SECURE_PORTAL"
  :prepared-at "2024-02-15T09:00:00Z")

;;; Result:
;;; - status: PRECHECKS → SUBSCRIPTION_DOCS
;;; - documents_package_id: "pkg-2024-001-docs"

;;; ----------------------------------------------------------------------------
;;; STATE 4: KYC
;;; Know Your Customer verification process
;;; State Transition: SUBSCRIPTION_DOCS → KYC
;;; ----------------------------------------------------------------------------

(kyc.verify
  :customer-id "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :customer-type "CORPORATE"
  :method "enhanced_due_diligence"
  :required-documents [
    "certificate_incorporation",
    "board_resolution",
    "beneficial_ownership_disclosure",
    "financial_statements"
  ]
  :verification-level "INSTITUTIONAL"
  :verified-at "2024-03-01T11:15:00Z")

(kyc.screen_sanctions
  :target "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :databases ["OFAC", "EU", "UN", "HMT"]
  :status "CLEAR"
  :screened-at "2024-03-01T11:30:00Z")

(kyc.check_pep
  :target "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :status "NOT_PEP"
  :checked-at "2024-03-01T11:45:00Z")

;;; Result:
;;; - status: SUBSCRIPTION_DOCS → KYC
;;; - kyc_status: "APPROVED"
;;; - compliance_level: "INSTITUTIONAL"

;;; ----------------------------------------------------------------------------
;;; STATE 5: SUBSCRIPTION
;;; Formal subscription processing
;;; State Transition: KYC → SUBSCRIPTION
;;; ----------------------------------------------------------------------------

(investor.process-subscription
  :investor-id "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :fund-id "f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c"
  :subscription-amount 5000000.00
  :currency "USD"
  :share-class "Class A USD"
  :subscription-date "2024-04-01"
  :settlement-date "2024-04-05"
  :payment-method "WIRE_TRANSFER"
  :processed-at "2024-03-20T16:00:00Z")

;;; Result:
;;; - status: KYC → SUBSCRIPTION
;;; - subscription_id: "sub-2024-001"
;;; - investor_shares: 50000.0

;;; ----------------------------------------------------------------------------
;;; STATE 6: SETTLEMENT
;;; Fund settlement and share allocation
;;; State Transition: SUBSCRIPTION → SETTLEMENT
;;; ----------------------------------------------------------------------------

(fund.settle-subscription
  :subscription-id "sub-2024-001"
  :investor-id "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :fund-id "f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c"
  :settlement-amount 5000000.00
  :shares-allocated 50000.0
  :nav-per-share 100.00
  :settlement-date "2024-04-05"
  :settled-at "2024-04-05T10:30:00Z")

;;; Result:
;;; - status: SUBSCRIPTION → SETTLEMENT
;;; - position_id: "pos-2024-001"
;;; - shares_outstanding: 50000.0

;;; ----------------------------------------------------------------------------
;;; STATE 7: ACTIVE INVESTOR
;;; Investor is now active in the fund
;;; State Transition: SETTLEMENT → ACTIVE
;;; ----------------------------------------------------------------------------

(investor.activate
  :investor-id "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :fund-id "f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c"
  :activation-date "2024-04-05"
  :reporting-frequency "MONTHLY"
  :distribution-preference "REINVEST"
  :activated-at "2024-04-05T15:00:00Z")

;;; Result:
;;; - status: SETTLEMENT → ACTIVE
;;; - investor_status: "ACTIVE"
;;; - first_valuation_date: "2024-04-30"

;;; ----------------------------------------------------------------------------
;;; ONGOING OPERATIONS: VALUATIONS AND REPORTING
;;; Monthly valuations and investor reporting
;;; ----------------------------------------------------------------------------

(fund.value-position
  :position-id "pos-2024-001"
  :valuation-date "2024-04-30"
  :nav-per-share 102.50
  :shares-outstanding 50000.0
  :position-value 5125000.00
  :performance-ytd 0.025
  :valued-at "2024-04-30T18:00:00Z")

(investor.generate-statement
  :investor-id "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :period-end "2024-04-30"
  :statement-type "MONTHLY"
  :includes [
    "POSITION_SUMMARY",
    "PERFORMANCE_ATTRIBUTION",
    "TRANSACTION_HISTORY",
    "CAPITAL_ACTIVITY"
  ]
  :generated-at "2024-05-01T09:00:00Z")

;;; ----------------------------------------------------------------------------
;;; STATE 8: REDEMPTION REQUEST
;;; Investor requests partial redemption
;;; State Transition: ACTIVE → REDEMPTION_PENDING
;;; ----------------------------------------------------------------------------

(investor.request-redemption
  :investor-id "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :fund-id "f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c"
  :redemption-type "PARTIAL"
  :redemption-amount 1000000.00
  :redemption-date "2024-10-31"
  :notice-period-days 90
  :requested-at "2024-08-01T14:00:00Z")

;;; Result:
;;; - status: ACTIVE → REDEMPTION_PENDING
;;; - redemption_id: "red-2024-001"
;;; - processing_date: "2024-10-31"

;;; ----------------------------------------------------------------------------
;;; STATE 9: REDEMPTION SETTLEMENT
;;; Process and settle the redemption
;;; State Transition: REDEMPTION_PENDING → REDEMPTION_SETTLED
;;; ----------------------------------------------------------------------------

(fund.settle-redemption
  :redemption-id "red-2024-001"
  :investor-id "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :fund-id "f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c"
  :redemption-amount 1000000.00
  :nav-per-share 105.00
  :shares-redeemed 9523.81
  :settlement-date "2024-10-31"
  :settled-at "2024-10-31T16:00:00Z")

;;; Result:
;;; - status: REDEMPTION_PENDING → REDEMPTION_SETTLED
;;; - remaining_shares: 40476.19
;;; - cash_distributed: 1000000.00

;;; ----------------------------------------------------------------------------
;;; STATE 10: FINAL REDEMPTION AND OFFBOARDING
;;; Complete redemption and investor offboarding
;;; State Transition: ACTIVE → OFFBOARDED
;;; ----------------------------------------------------------------------------

(investor.complete-redemption
  :investor-id "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :fund-id "f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c"
  :final-redemption-date "2024-12-31"
  :final-shares 40476.19
  :final-nav-per-share 107.25
  :final-amount 4341069.87
  :completed-at "2024-12-31T17:00:00Z")

(investor.offboard
  :investor-id "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :fund-id "f1a2b3c4-d5e6-4f5a-9b8c-7d6e5f4a3b2c"
  :offboard-reason "COMPLETE_REDEMPTION"
  :final-tax-documents ["1099", "K1"]
  :record-retention-years 7
  :offboarded-at "2024-12-31T18:00:00Z")

;;; Result:
;;; - status: ACTIVE → OFFBOARDED
;;; - final_position_value: 0.00
;;; - total_return: 7.25%
;;; - relationship_duration: 11_months

;;; ----------------------------------------------------------------------------
;;; COMPLIANCE AND AUDIT TRAIL
;;; Final compliance verification and audit trail completion
;;; ----------------------------------------------------------------------------

(compliance.verify
  :framework "SEC_INVESTMENT_ADVISER"
  :jurisdiction "US"
  :investor-id "a1b2c3d4-e5f6-4a5b-8c9d-0e1f2a3b4c5d"
  :status "COMPLIANT"
  :checks [
    "investor_suitability",
    "aml_compliance",
    "tax_reporting",
    "record_keeping"
  ]
  :verified-at "2024-12-31T19:00:00Z")

(workflow.transition
  :from "ACTIVE"
  :to "OFFBOARDED"
  :reason "Complete investor lifecycle - subscription to full redemption"
  :timestamp "2024-12-31T20:00:00Z"
  :metadata {
    :total-invested 5000000.00
    :total-redeemed 5341069.87
    :total-return-pct 6.82
    :duration-months 11
    :transactions-count 4
  })

;;; ============================================================================
;;; HEDGE FUND INVESTOR LIFECYCLE COMPLETE
;;; ============================================================================
;;;
;;; Final Status: OFFBOARDED
;;; Total Investment: $5,000,000 USD
;;; Total Return: $341,069.87 (6.82%)
;;; Duration: 11 months (Jan 2024 - Dec 2024)
;;;
;;; State Flow Summary:
;;; OPPORTUNITY → PRECHECKS → SUBSCRIPTION_DOCS → KYC → SUBSCRIPTION →
;;; SETTLEMENT → ACTIVE → REDEMPTION_PENDING → REDEMPTION_SETTLED → OFFBOARDED
;;;
;;; This example demonstrates:
;;; ✅ Complete investor lifecycle management
;;; ✅ v3.0 EBNF compliant syntax throughout
;;; ✅ Proper state transitions with timestamps
;;; ✅ Comprehensive KYC and compliance checks
;;; ✅ Fund operations (subscriptions, redemptions, valuations)
;;; ✅ Audit trail and regulatory compliance
;;; ✅ DSL-as-State pattern with accumulated document history
;;; ============================================================================
