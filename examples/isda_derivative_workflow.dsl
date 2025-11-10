;; ISDA Derivative Workflow Example
;; Complete lifecycle of derivative trading under ISDA Master Agreement
;; Demonstrates integration between Document Library and ISDA DSL domains
;;
;; Scenario: Zenith Capital SPV enters into interest rate swap with JPMorgan
;; Timeline: Master Agreement → CSA → Trade Execution → Collateral Management → Valuation

;; ============================================================================
;; DOCUMENT CATALOGING - Legal Documentation Foundation
;; ============================================================================

;; Catalog the ISDA Master Agreement document
(document.catalog
  :document-id "doc-isda-master-zenith-jpm-001"
  :document-type "ISDA_MASTER_AGREEMENT"
  :issuer "isda_inc"
  :title "ISDA Master Agreement - Zenith Capital SPV / JPMorgan Chase"
  :parties ["company-zenith-spv-001" "jpmorgan-chase-entity"]
  :document-date "2023-01-15"
  :jurisdiction "NY"
  :language "EN"
  :confidentiality-level "RESTRICTED"
  :extracted-data {
    :isda.governing_law "NY"
    :isda.master_agreement_version "2002"
    :isda.multicurrency_cross_default true
    :isda.cross_default_threshold 5000000
    :isda.termination_currency "USD"
  })

;; Catalog the Credit Support Annex
(document.catalog
  :document-id "doc-csa-zenith-jpm-001"
  :document-type "CREDIT_SUPPORT_ANNEX"
  :issuer "cleary_gottlieb"
  :title "Credit Support Annex - USD Variation Margin"
  :parties ["company-zenith-spv-001" "jpmorgan-chase-entity"]
  :document-date "2023-01-15"
  :jurisdiction "NY"
  :confidentiality-level "RESTRICTED"
  :extracted-data {
    :isda.csa_base_currency "USD"
    :isda.threshold_party_a 0
    :isda.threshold_party_b 5000000
    :isda.minimum_transfer_amount 100000
    :isda.eligible_collateral ["cash_usd" "us_treasury_bills" "us_treasury_notes"]
    :isda.margin_approach "VM"
  })

;; Verify document authenticity
(document.verify
  :document-id "doc-isda-master-zenith-jpm-001"
  :verification-method "digital_signature"
  :verifier "cleary_gottlieb_verification_service"
  :verification-date "2023-01-16"
  :verification-result "AUTHENTIC"
  :verification-details {
    :signature-valid true
    :document-integrity true
    :issuer-authorized true
  })

;; ============================================================================
;; ISDA MASTER AGREEMENT ESTABLISHMENT
;; ============================================================================

;; Establish ISDA Master Agreement between counterparties
(isda.establish_master
  :agreement-id "ISDA-ZENITH-JPM-001"
  :party-a "company-zenith-spv-001"
  :party-b "jpmorgan-chase-entity"
  :version "2002"
  :governing-law "NY"
  :agreement-date "2023-01-15"
  :effective-date "2023-01-15"
  :multicurrency true
  :cross-default true
  :termination-currency "USD"
  :document-id "doc-isda-master-zenith-jpm-001")

;; Link the cataloged document to the master agreement
(document.link
  :primary-document "doc-isda-master-zenith-jpm-001"
  :related-document "doc-csa-zenith-jpm-001"
  :relationship-type "GOVERNED_BY"
  :relationship-description "CSA operates under Master Agreement framework")

;; ============================================================================
;; CREDIT SUPPORT ANNEX SETUP
;; ============================================================================

;; Establish Credit Support Annex for collateral management
(isda.establish_csa
  :csa-id "CSA-ZENITH-JPM-001"
  :master-agreement-id "ISDA-ZENITH-JPM-001"
  :base-currency "USD"
  :threshold-party-a 0
  :threshold-party-b 5000000
  :minimum-transfer 100000
  :rounding-amount 10000
  :eligible-collateral ["cash_usd" "us_treasury_bills" "us_treasury_notes"]
  :valuation-percentage {
    "cash_usd" 100
    "us_treasury_bills" 98
    "us_treasury_notes" 95
  }
  :margin-approach "VM"
  :effective-date "2023-01-15"
  :document-id "doc-csa-zenith-jpm-001")

;; Track CSA document usage in workflow
(document.use
  :document-id "doc-csa-zenith-jpm-001"
  :used-by-process "ISDA_COLLATERAL_SETUP"
  :usage-date "2023-01-15"
  :usage-context "CSA establishment for derivative collateral management"
  :business-purpose "RISK_MANAGEMENT")

;; ============================================================================
;; DERIVATIVE TRADE EXECUTION
;; ============================================================================

;; Execute 5-year USD interest rate swap
(isda.execute_trade
  :trade-id "TRADE-IRS-ZENITH-JPM-001"
  :master-agreement-id "ISDA-ZENITH-JPM-001"
  :product-type "IRS"
  :trade-date "2024-03-15"
  :effective-date "2024-03-17"
  :termination-date "2029-03-17"
  :notional-amount 50000000
  :currency "USD"
  :payer "company-zenith-spv-001"
  :receiver "jpmorgan-chase-entity"
  :underlying "USD-SOFR"
  :calculation-agent "jpmorgan-chase-entity"
  :settlement-terms {
    :payment-frequency "QUARTERLY"
    :day-count-convention "ACT/360"
    :business-day-convention "MODIFIED_FOLLOWING"
    :reset-frequency "QUARTERLY"
    :fixed-rate 0.0425
  })

;; Catalog trade confirmation document
(document.catalog
  :document-id "doc-confirmation-irs-001"
  :document-type "TRADE_CONFIRMATION"
  :issuer "jpmorgan_chase_bank"
  :title "IRS Trade Confirmation - TRADE-IRS-ZENITH-JPM-001"
  :parties ["company-zenith-spv-001" "jpmorgan-chase-entity"]
  :document-date "2024-03-15"
  :confidentiality-level "CONFIDENTIAL"
  :extracted-data {
    :isda.trade_id "TRADE-IRS-ZENITH-JPM-001"
    :isda.product_type "IRS"
    :isda.notional_amount 50000000
    :isda.currency "USD"
    :isda.fixed_rate 0.0425
    :isda.underlying_rate "USD-SOFR"
  })

;; ============================================================================
;; PORTFOLIO VALUATION & RISK MANAGEMENT
;; ============================================================================

;; Daily portfolio valuation (6 months later - rates have moved)
(isda.value_portfolio
  :valuation-id "VAL-ZENITH-JPM-20240915"
  :portfolio-id "PORTFOLIO-ZENITH-JPM"
  :valuation-date "2024-09-15"
  :valuation-agent "jpmorgan-chase-entity"
  :methodology "MARKET_STANDARD"
  :base-currency "USD"
  :trades-valued ["TRADE-IRS-ZENITH-JPM-001"]
  :gross-mtm -8750000
  :net-mtm -8750000
  :market-data-sources ["Bloomberg" "Refinitiv"]
  :calculation-details {
    :sofr-curve "USD-SOFR-CURVE-20240915"
    :discount-curve "USD-OIS-CURVE-20240915"
    :volatility-surface "USD-SWAPTION-VOL-20240915"
  })

;; ============================================================================
;; COLLATERAL MANAGEMENT
;; ============================================================================

;; Issue margin call due to negative MTM exceeding threshold
(isda.margin_call
  :call-id "MC-ZENITH-JPM-20240915"
  :csa-id "CSA-ZENITH-JPM-001"
  :call-date "2024-09-15"
  :valuation-date "2024-09-15"
  :calling-party "jpmorgan-chase-entity"
  :called-party "company-zenith-spv-001"
  :exposure-amount 8750000
  :existing-collateral 3000000
  :call-amount 5700000  ;; Rounded to nearest 10k per CSA terms
  :currency "USD"
  :deadline "2024-09-16T17:00:00Z"
  :calculation-details {
    :threshold-amount 5000000
    :minimum-transfer 100000
    :rounding-amount 10000
    :valuation-percentage 100
  })

;; Post collateral in response to margin call
(isda.post_collateral
  :posting-id "POST-ZENITH-JPM-20240916"
  :call-id "MC-ZENITH-JPM-20240915"
  :posting-party "company-zenith-spv-001"
  :receiving-party "jpmorgan-chase-entity"
  :collateral-type "cash_usd"
  :amount 5700000
  :currency "USD"
  :posting-date "2024-09-16"
  :settlement-date "2024-09-16"
  :custodian "jpmorgan-chase-custody"
  :valuation 5700000)

;; ============================================================================
;; TRADE AMENDMENTS & MODIFICATIONS
;; ============================================================================

;; Amend the Master Agreement to update threshold amounts
(isda.amend_agreement
  :amendment-id "AMEND-ZENITH-JPM-001"
  :original-agreement-id "ISDA-ZENITH-JPM-001"
  :amendment-type "CSA_MODIFICATION"
  :amendment-date "2024-06-15"
  :effective-date "2024-07-01"
  :sections-amended ["Part 4(h)" "Part 5(a)"]
  :amendment-description "Updated threshold amounts: Party A from $0 to $2M, Party B from $5M to $10M"
  :party-a-consent true
  :party-b-consent true
  :supersedes-prior false)

;; Catalog amendment document
(document.catalog
  :document-id "doc-amendment-001"
  :document-type "AMENDMENT_LETTER"
  :issuer "cleary_gottlieb"
  :title "Amendment to ISDA Master Agreement - Threshold Updates"
  :parties ["company-zenith-spv-001" "jpmorgan-chase-entity"]
  :document-date "2024-06-15"
  :confidentiality-level "RESTRICTED"
  :extracted-data {
    :isda.amendment_type "CSA_MODIFICATION"
    :isda.new_threshold_party_a 2000000
    :isda.new_threshold_party_b 10000000
    :isda.effective_date "2024-07-01"
  })

;; ============================================================================
;; NETTING SET MANAGEMENT
;; ============================================================================

;; Manage netting set for exposure calculations (after executing second trade)
(isda.execute_trade
  :trade-id "TRADE-IRS-ZENITH-JPM-002"
  :master-agreement-id "ISDA-ZENITH-JPM-001"
  :product-type "IRS"
  :trade-date "2024-10-15"
  :effective-date "2024-10-17"
  :termination-date "2027-10-17"
  :notional-amount 25000000
  :currency "USD"
  :payer "jpmorgan-chase-entity"  ;; Opposite direction
  :receiver "company-zenith-spv-001"
  :underlying "USD-SOFR"
  :calculation-agent "jpmorgan-chase-entity"
  :settlement-terms {
    :payment-frequency "QUARTERLY"
    :day-count-convention "ACT/360"
    :business-day-convention "MODIFIED_FOLLOWING"
    :reset-frequency "QUARTERLY"
    :fixed-rate 0.0475
  })

;; Calculate netting benefits across portfolio
(isda.manage_netting_set
  :netting-set-id "NETTING-ZENITH-JPM"
  :master-agreement-id "ISDA-ZENITH-JPM-001"
  :included-trades ["TRADE-IRS-ZENITH-JPM-001" "TRADE-IRS-ZENITH-JPM-002"]
  :netting-date "2024-11-15"
  :gross-exposure 12500000  ;; Sum of absolute values
  :net-exposure 6250000     ;; After netting opposite positions
  :currency "USD"
  :calculation-method "STANDARD_NETTING"
  :legal-opinion "doc-netting-opinion-cleary-001")

;; ============================================================================
;; DOCUMENT LIFECYCLE MANAGEMENT
;; ============================================================================

;; Extract key data from confirmation for regulatory reporting
(document.extract
  :document-id "doc-confirmation-irs-001"
  :extraction-method "AUTOMATED_OCR"
  :extracted-date "2024-03-15"
  :extractor "ISDA_DOCUMENT_PROCESSOR"
  :extracted-attributes {
    :isda.trade_id "TRADE-IRS-ZENITH-JPM-001"
    :isda.notional_amount 50000000
    :isda.currency "USD"
    :isda.product_type "IRS"
    :isda.effective_date "2024-03-17"
    :isda.termination_date "2029-03-17"
    :isda.fixed_rate 0.0425
  }
  :confidence-score 0.98)

;; Query document library for regulatory reporting
(document.query
  :query-id "QUERY-REGULATORY-EMIR-001"
  :query-type "REGULATORY_REPORTING"
  :search-criteria {
    :document-types ["TRADE_CONFIRMATION" "ISDA_MASTER_AGREEMENT"]
    :parties ["company-zenith-spv-001"]
    :date-range ["2024-01-01" "2024-12-31"]
    :jurisdictions ["NY" "EU"]
  }
  :output-format "EMIR_XML"
  :regulatory-framework "EMIR")

;; ============================================================================
;; WORKFLOW SUMMARY & AUDIT TRAIL
;; ============================================================================

;; This comprehensive workflow demonstrates:
;; 1. Document Library Integration - Cataloging, verification, and lifecycle management
;; 2. ISDA Master Agreement establishment with proper legal documentation
;; 3. Credit Support Annex setup for collateral management
;; 4. Derivative trade execution with confirmation processing
;; 5. Risk management through portfolio valuation and margin calls
;; 6. Collateral posting in response to margin requirements
;; 7. Agreement amendments with proper documentation
;; 8. Netting set management for exposure optimization
;; 9. Automated data extraction for regulatory compliance
;; 10. Cross-domain integration between Document and ISDA domains

;; Key Benefits Demonstrated:
;; - Complete audit trail from legal documents to trade execution
;; - AttributeID-typed data extraction ensuring type safety
;; - Multi-domain workflow coordination
;; - Regulatory compliance through structured documentation
;; - AI-ready document processing with confidence scoring
;; - Risk management through automated valuation and collateral management

;; This DSL workflow serves as the state representation, audit log,
;; and executable specification for the entire derivative lifecycle.
