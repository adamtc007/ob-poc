;; DSL v3.1 Example: ISDA Derivative Workflow - Zenith Capital Hedge Fund
;; This example demonstrates the new ISDA domain integration with document library
;; and comprehensive derivative contract lifecycle management

(define-kyc-investigation
  :id "zenith-capital-derivatives-onboarding"
  :target-entity "company-zenith-spv-001"
  :jurisdiction "KY"
  :ubo-threshold 25.0

  ;; 1. ENTITY SETUP - Counterparties for derivative trading
  (entity
    :id "company-zenith-spv-001"
    :label "Company"
    :props {
      :legal-name "Zenith Capital Partners LP"
      :registration-number "KY-123456"
      :jurisdiction "KY"
      :entity-type "Limited Partnership"
      :business-type "Hedge Fund"
      :regulatory-status "Unregulated"
    })

  (entity
    :id "jpmorgan-chase-entity"
    :label "Company"
    :props {
      :legal-name "JPMorgan Chase Bank, N.A."
      :registration-number "US-FDIC-628"
      :jurisdiction "US"
      :entity-type "National Bank"
      :business-type "Prime Broker"
      :regulatory-status "FDIC Insured"
    })

  (entity
    :id "goldman-sachs-entity"
    :label "Company"
    :props {
      :legal-name "Goldman Sachs International"
      :registration-number "GB-FCA-142987"
      :jurisdiction "GB"
      :entity-type "Investment Bank"
      :business-type "Market Maker"
      :regulatory-status "FCA Authorized"
    })

  ;; 2. DOCUMENT LIBRARY - Catalog all ISDA-related documents
  (document.catalog
    :doc-id "doc-isda-master-zenith-jpm-001"
    :doc-type "isda_master_agreement"
    :issuer "jpmorgan_chase"
    :title "ISDA 2002 Master Agreement - Zenith Capital Partners LP and JPMorgan Chase Bank, N.A."
    :issue-date "2023-01-15"
    :related-entities ["company-zenith-spv-001", "jpmorgan-chase-entity"]
    :tags ["isda", "master_agreement", "derivatives", "prime_brokerage", "hedge_fund"]
    :confidentiality "confidential"
    :description "ISDA 2002 Master Agreement governing all OTC derivative transactions between Zenith Capital and JPMorgan Chase")

  (document.catalog
    :doc-id "doc-isda-csa-zenith-jpm-001"
    :doc-type "isda_csa"
    :issuer "jpmorgan_chase"
    :title "Credit Support Annex - Zenith Capital Partners LP and JPMorgan Chase Bank, N.A."
    :issue-date "2023-01-15"
    :related-entities ["company-zenith-spv-001", "jpmorgan-chase-entity"]
    :tags ["isda", "csa", "collateral", "margin", "credit_support"]
    :confidentiality "confidential"
    :description "Credit Support Annex defining collateral arrangements and margin requirements")

  (document.catalog
    :doc-id "doc-isda-master-zenith-gs-001"
    :doc-type "isda_master_agreement"
    :issuer "goldman_sachs"
    :title "ISDA 2002 Master Agreement - Zenith Capital Partners LP and Goldman Sachs International"
    :issue-date "2023-02-01"
    :related-entities ["company-zenith-spv-001", "goldman-sachs-entity"]
    :tags ["isda", "master_agreement", "derivatives", "market_making"]
    :confidentiality "confidential"
    :description "ISDA 2002 Master Agreement for derivatives trading with Goldman Sachs")

  (document.catalog
    :doc-id "doc-netting-opinion-ky"
    :doc-type "isda_netting_opinion"
    :issuer "allen_overy"
    :title "Cayman Islands Netting Opinion - Limited Partnerships"
    :issue-date "2022-01-01"
    :expiry-date "2027-01-01"
    :related-entities ["company-zenith-spv-001"]
    :tags ["netting", "legal_opinion", "cayman_islands", "enforceability"]
    :confidentiality "internal"
    :description "Legal opinion on enforceability of close-out netting for Cayman Islands limited partnerships")

  ;; 3. DOCUMENT VERIFICATION AND EXTRACTION
  (document.verify
    :doc-id "doc-isda-master-zenith-jpm-001"
    :verification-method "legal_review"
    :verifier "legal_department"
    :status "verified"
    :confidence 1.0
    :verified-at "2023-01-20T14:30:00Z"
    :notes "Legal review completed, all terms standard, executed copies on file")

  (document.extract
    :doc-id "doc-isda-master-zenith-jpm-001"
    :extraction-method "manual"
    :extracted-fields {
      :agreement-date "2023-01-15"
      :party-a "Zenith Capital Partners LP"
      :party-b "JPMorgan Chase Bank, N.A."
      :governing-law "NY"
      :version "2002"
      :multicurrency true
      :cross-default true
      :termination-currency "USD"
      :threshold-amount 10000000
    }
    :confidence 1.0
    :extracted-at "2023-01-20T15:00:00Z"
    :extracted-by "legal_department")

  (document.extract
    :doc-id "doc-isda-csa-zenith-jpm-001"
    :extraction-method "manual"
    :extracted-fields {
      :base-currency "USD"
      :threshold-party-a 0
      :threshold-party-b 5000000
      :minimum-transfer 100000
      :rounding-amount 10000
      :eligible-collateral ["cash_usd", "us_treasury", "us_agency"]
      :valuation-percentage {"cash_usd": 1.0, "us_treasury": 0.98, "us_agency": 0.95}
      :margin-approach "VM"
      :notification-time "11:00 AM New York time"
    }
    :confidence 1.0
    :extracted-at "2023-01-20T15:30:00Z"
    :extracted-by "operations_department")

  ;; 4. ISDA MASTER AGREEMENT ESTABLISHMENT
  (isda.establish_master
    :agreement-id "ISDA-ZENITH-JPM-001"
    :party-a "company-zenith-spv-001"
    :party-b "jpmorgan-chase-entity"
    :version "2002"
    :governing-law "NY"
    :agreement-date "2023-01-15"
    :multicurrency true
    :cross-default true
    :termination-currency "USD"
    :document-id "doc-isda-master-zenith-jpm-001"
    :effective-date "2023-01-15")

  (isda.establish_master
    :agreement-id "ISDA-ZENITH-GS-001"
    :party-a "company-zenith-spv-001"
    :party-b "goldman-sachs-entity"
    :version "2002"
    :governing-law "NY"
    :agreement-date "2023-02-01"
    :multicurrency true
    :cross-default true
    :termination-currency "USD"
    :document-id "doc-isda-master-zenith-gs-001"
    :effective-date "2023-02-01")

  ;; 5. CREDIT SUPPORT ANNEX ESTABLISHMENT
  (isda.establish_csa
    :csa-id "CSA-ZENITH-JPM-001"
    :master-agreement-id "ISDA-ZENITH-JPM-001"
    :base-currency "USD"
    :threshold-party-a 0
    :threshold-party-b 5000000
    :minimum-transfer 100000
    :rounding-amount 10000
    :eligible-collateral ["cash_usd", "us_treasury", "us_agency"]
    :valuation-percentage {
      :cash_usd 1.0
      :us_treasury 0.98
      :us_agency 0.95
    }
    :margin-approach "VM"
    :document-id "doc-isda-csa-zenith-jpm-001"
    :effective-date "2023-01-15")

  ;; 6. DOCUMENT RELATIONSHIPS
  (document.link
    :source-doc "doc-isda-csa-zenith-jpm-001"
    :target-doc "doc-isda-master-zenith-jpm-001"
    :relationship "supports"
    :strength "strong"
    :description "CSA provides collateral framework for Master Agreement"
    :effective-date "2023-01-15")

  (document.link
    :source-doc "doc-netting-opinion-ky"
    :target-doc "doc-isda-master-zenith-jpm-001"
    :relationship "supports"
    :strength "strong"
    :description "Legal opinion supports netting enforceability for Cayman entity"
    :effective-date "2022-01-01")

  ;; 7. DERIVATIVE TRADE EXECUTION
  (isda.execute_trade
    :trade-id "TRADE-IRS-ZENITH-001"
    :master-agreement-id "ISDA-ZENITH-JPM-001"
    :product-type "IRS"
    :trade-date "2024-03-15"
    :effective-date "2024-03-17"
    :termination-date "2029-03-17"
    :notional-amount 50000000
    :currency "USD"
    :payer "company-zenith-spv-001"
    :receiver "jpmorgan-chase-entity"
    :underlying "USD-SOFR-3M"
    :calculation-agent "jpmorgan-chase-entity"
    :settlement-terms {
      :payment-frequency "quarterly"
      :day-count "ACT/360"
      :business-day-convention "Modified Following"
      :reset-frequency "quarterly"
    }
    :confirmation-id "doc-irs-confirm-zenith-001")

  (isda.execute_trade
    :trade-id "TRADE-CDS-ZENITH-001"
    :master-agreement-id "ISDA-ZENITH-GS-001"
    :product-type "CDS"
    :trade-date "2024-04-10"
    :effective-date "2024-04-12"
    :termination-date "2029-04-12"
    :notional-amount 25000000
    :currency "USD"
    :payer "company-zenith-spv-001"
    :receiver "goldman-sachs-entity"
    :underlying "CDX.NA.IG.42"
    :calculation-agent "goldman-sachs-entity"
    :settlement-terms {
      :premium-rate 1.25
      :payment-frequency "quarterly"
      :recovery-rate 0.40
      :restructuring "ModR"
    }
    :confirmation-id "doc-cds-confirm-zenith-001")

  ;; 8. DOCUMENT USAGE TRACKING
  (document.use
    :doc-id "doc-isda-master-zenith-jpm-001"
    :usage-type "evidence"
    :workflow-stage "trade_execution"
    :verb-context "isda.execute_trade"
    :cbu-id "CBU-ZENITH-001"
    :purpose "Legal framework for IRS execution"
    :outcome "successful"
    :used-by "trading_system")

  (document.use
    :doc-id "doc-isda-csa-zenith-jpm-001"
    :usage-type "reference"
    :workflow-stage "collateral_management"
    :verb-context "isda.margin_call"
    :purpose "Collateral terms reference for margin calculations")

  ;; 9. PORTFOLIO VALUATION
  (isda.value_portfolio
    :valuation-id "VAL-ZENITH-20241115"
    :portfolio-id "PORTFOLIO-ZENITH-DERIVATIVES"
    :valuation-date "2024-11-15"
    :valuation-agent "jpmorgan-chase-entity"
    :methodology "market_standard"
    :base-currency "USD"
    :trades-valued ["TRADE-IRS-ZENITH-001", "TRADE-CDS-ZENITH-001"]
    :gross-mtm 8750000
    :net-mtm 8500000
    :market-data-sources ["Bloomberg", "Refinitiv", "ICE"]
    :calculation-details {
      :irs-mtm 6500000
      :cds-mtm 2250000
      :fx-adjustment -250000
    })

  ;; 10. MARGIN CALL PROCESS
  (isda.margin_call
    :call-id "MC-ZENITH-20241115"
    :csa-id "CSA-ZENITH-JPM-001"
    :call-date "2024-11-15"
    :valuation-date "2024-11-14"
    :calling-party "jpmorgan-chase-entity"
    :called-party "company-zenith-spv-001"
    :exposure-amount 8500000
    :existing-collateral 3000000
    :call-amount 5000000
    :currency "USD"
    :deadline "2024-11-16T17:00:00Z"
    :calculation-details {
      :gross-exposure 8750000
      :netting-benefit 250000
      :net-exposure 8500000
      :threshold 5000000
      :minimum-transfer 100000
      :rounding 10000
      :final-call 5000000
    })

  ;; 11. COLLATERAL POSTING
  (isda.post_collateral
    :posting-id "POST-ZENITH-20241116"
    :call-id "MC-ZENITH-20241115"
    :posting-party "company-zenith-spv-001"
    :receiving-party "jpmorgan-chase-entity"
    :collateral-type "cash_usd"
    :amount 5000000
    :currency "USD"
    :posting-date "2024-11-16"
    :settlement-date "2024-11-16"
    :custodian "jpmorgan-chase-custody"
    :valuation 5000000)

  ;; 12. AMENDMENT EXAMPLE
  (isda.amend_agreement
    :amendment-id "AMEND-ZENITH-JPM-001"
    :original-agreement-id "ISDA-ZENITH-JPM-001"
    :amendment-type "csa_modification"
    :amendment-date "2024-06-15"
    :effective-date "2024-07-01"
    :sections-amended ["CSA Paragraph 11(c)", "CSA Paragraph 13"]
    :amendment-description "Updated threshold amounts to reflect increased trading volume and added digital assets to eligible collateral"
    :party-a-consent true
    :party-b-consent true
    :document-id "doc-amendment-zenith-jpm-001"
    :supersedes-prior false)

  ;; 13. DOCUMENT AMENDMENT TRACKING
  (document.amend
    :parent-doc "doc-isda-csa-zenith-jpm-001"
    :new-doc-id "doc-amendment-zenith-jpm-001"
    :amendment-type "modification"
    :changes ["Increased threshold from $5M to $10M", "Added BTC and ETH to eligible collateral"]
    :effective-date "2024-07-01"
    :supersedes-parent false
    :amended-by "legal_department")

  ;; 14. NETTING SET MANAGEMENT
  (isda.manage_netting_set
    :netting-set-id "NETTING-ZENITH-JPM"
    :master-agreement-id "ISDA-ZENITH-JPM-001"
    :included-trades ["TRADE-IRS-ZENITH-001"]
    :netting-date "2024-11-15"
    :gross-exposure 6500000
    :net-exposure 6500000
    :currency "USD"
    :calculation-method "mark_to_market"
    :legal-opinion "doc-netting-opinion-ky")

  ;; 15. RISK MONITORING AND COMPLIANCE
  (compliance.verify
    :entity "company-zenith-spv-001"
    :framework "EMIR"
    :jurisdiction "EU"
    :status "COMPLIANT"
    :checks ["trade_reporting", "clearing_obligation", "risk_mitigation"]
    :verified-at "2024-11-15T16:00:00Z"
    :notes "All derivative trades properly reported to trade repository")

  (kyc.screen_sanctions
    :target "goldman-sachs-entity"
    :databases ["OFAC", "UN", "EU", "HMT"]
    :status "CLEAR"
    :screened-at "2024-11-15T16:30:00Z"
    :notes "Counterparty screening clear for derivative trading relationship")

  ;; 16. HYPOTHETICAL TERMINATION EVENT SCENARIO
  (isda.declare_termination_event
    :event-id "TERM-EVENT-ZENITH-001"
    :master-agreement-id "ISDA-ZENITH-JPM-001"
    :event-type "failure_to_pay_or_deliver"
    :affected-party "company-zenith-spv-001"
    :declaring-party "jpmorgan-chase-entity"
    :event-date "2024-12-01"
    :declaration-date "2024-12-02"
    :cure-period 3
    :event-description "Failure to post required additional collateral within specified timeframe following margin call MC-ZENITH-20241115"
    :supporting-evidence ["doc-margin-call-notice", "doc-collateral-shortfall"]
    :automatic false)

  ;; 17. CLOSE-OUT CALCULATION (HYPOTHETICAL)
  (isda.close_out
    :closeout-id "CLOSEOUT-ZENITH-001"
    :master-agreement-id "ISDA-ZENITH-JPM-001"
    :termination-date "2024-12-05"
    :calculation-agent "jpmorgan-chase-entity"
    :terminated-trades ["TRADE-IRS-ZENITH-001"]
    :valuation-method "market_quotation"
    :market-quotations ["quote-dealer-1", "quote-dealer-2", "quote-dealer-3"]
    :loss-calculation "first_method"
    :closeout-amount 2750000
    :payment-currency "USD"
    :payment-date "2024-12-07"
    :calculation-statement "doc-closeout-statement-001")

  ;; 18. FINAL CASE STATUS UPDATE
  (case.update
    :id "CBU-ZENITH-001"
    :status "DERIVATIVES_ACTIVE"
    :add-products ["DERIVATIVES_TRADING", "PRIME_BROKERAGE"]
    :add-services ["MARGIN_FINANCING", "SECURITIES_LENDING"]
    :updated-at "2024-11-15T17:00:00Z"
    :notes "ISDA Master Agreements established with JPMorgan and Goldman Sachs, active derivative trading commenced")

  ;; 19. WORKFLOW TRANSITION
  (workflow.transition
    :from "DERIVATIVES_SETUP"
    :to "ACTIVE_TRADING"
    :reason "All ISDA documentation executed, initial trades completed, risk management framework operational"
    :timestamp "2024-11-15T17:30:00Z")

  ;; 20. DOCUMENT QUERY EXAMPLE FOR AI/RAG
  (document.query
    :search-criteria {
      :doc-type "isda_master_agreement"
      :related-entities ["company-zenith-spv-001"]
      :tags ["derivatives"]
      :confidentiality "confidential"
    }
    :result-limit 10
    :include-expired false
    :sort-by "issue-date"
    :context "compliance_review"))
