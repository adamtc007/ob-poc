;; Master Onboarding Request with Embedded Domain DSLs
;; OB Request ID: CBU-2025-001
;; Demonstrates hierarchical DSL structure with domain sections

;; ============================================================================
;; MASTER ONBOARDING REQUEST
;; ============================================================================

(onboarding.request
  :ob-id "CBU-2025-001"
  :entity-name "Zenith Capital Partners LP"
  :entity-type "HEDGE_FUND"
  :jurisdiction "KY"
  :aum 750000000.0
  :prime-broker "Goldman Sachs"
  :administrator "SS&C"
  :requested-services ["CUSTODY" "FUND_ACCOUNTING" "DERIVATIVES"])

;; ============================================================================
;; KYC DOMAIN SECTION
;; ============================================================================

(domain.section :name "kyc"
  ;; Entity KYC verification
  (kyc.verify
    :entity-id "zenith-capital-lp"
    :verification-level "ENHANCED"
    :risk-rating "MEDIUM"
    :documents ["certificate-incorporation" "memorandum-association" "partnership-agreement"]
    :jurisdictions ["KY" "US"]
    :beneficial-owners ["john-smith" "mary-jones"])

  ;; UBO discovery and verification
  (kyc.ubo-discovery
    :target-entity "zenith-capital-lp"
    :ownership-threshold 25.0
    :verification-required true
    :source-documents ["partnership-register" "shareholder-registry"])

  ;; Risk assessment
  (kyc.risk-assessment
    :entity-id "zenith-capital-lp"
    :risk-level "MEDIUM"
    :pep-screening "PASSED"
    :sanctions-screening "PASSED"
    :aml-checks "COMPLETED"
    :source-of-funds "INSTITUTIONAL"))

;; ============================================================================
;; DOCUMENT DOMAIN SECTION
;; ============================================================================

(domain.section :name "document"
  ;; Incorporation documents
  (document.catalog
    :document-id "doc-incorporation-ky-001"
    :document-type "INCORPORATION"
    :issuer "cayman-registry"
    :title "Certificate of Incorporation"
    :parties ["zenith-capital-lp"]
    :jurisdiction "KY"
    :confidentiality-level "CONFIDENTIAL")

  (document.catalog
    :document-id "doc-partnership-agreement"
    :document-type "PARTNERSHIP_AGREEMENT"
    :issuer "maples-fiduciary"
    :title "Limited Partnership Agreement"
    :parties ["zenith-capital-lp" "zenith-gp-ltd"]
    :jurisdiction "KY")

  ;; Document verification
  (document.verify
    :document-id "doc-incorporation-ky-001"
    :verification-method "APOSTILLE"
    :verification-result "AUTHENTIC"
    :verified-by "cayman-government"
    :verification-date "2025-01-15")

  ;; Extract key data for onboarding
  (document.extract
    :document-id "doc-partnership-agreement"
    :extraction-target "ENTITY_DETAILS"
    :extracted-data {
      :legal-name "Zenith Capital Partners LP"
      :registration-number "KY-123456-LP"
      :registered-address "Grand Cayman, KY"
      :general-partner "Zenith GP Ltd"
    }))

;; ============================================================================
;; ISDA DOMAIN SECTION
;; ============================================================================

(domain.section :name "isda"
  ;; Establish ISDA Master Agreement
  (isda.establish_master
    :agreement-id "ISDA-ZENITH-GS-001"
    :counterparty "goldman-sachs"
    :agreement-type "2002_MASTER"
    :governing-law "NY"
    :netting-eligible true
    :calculation-agent "goldman-sachs")

  ;; Credit Support Annex
  (isda.establish_csa
    :csa-id "CSA-ZENITH-GS-001"
    :master-agreement-id "ISDA-ZENITH-GS-001"
    :collateral-type "CASH_SECURITIES"
    :threshold-amount 5000000.0
    :minimum-transfer 250000.0
    :currency "USD")

  ;; Initial trade execution
  (isda.execute_trade
    :trade-id "TRD-ZENITH-001"
    :master-agreement-id "ISDA-ZENITH-GS-001"
    :trade-type "EQUITY_SWAP"
    :notional 10000000.0
    :underlying "SPY"
    :effective-date "2025-01-20"
    :maturity-date "2025-07-20"))

;; ============================================================================
;; COMPLIANCE DOMAIN SECTION
;; ============================================================================

(domain.section :name "compliance"
  ;; Regulatory compliance checks
  (compliance.check
    :entity-id "zenith-capital-lp"
    :regulation "CFTC"
    :status "COMPLIANT"
    :registration-required false
    :exemption-claimed "3C1_PRIVATE_FUND")

  (compliance.check
    :entity-id "zenith-capital-lp"
    :regulation "SEC"
    :status "REGISTERED"
    :registration-number "801-123456"
    :filing-requirements ["FORM_ADV" "FORM_PF"])

  ;; FATCA/CRS compliance
  (compliance.fatca-classification
    :entity-id "zenith-capital-lp"
    :classification "NON_US_ENTITY"
    :giin "ABC123.KY.456"
    :reporting-required true
    :reporting-jurisdiction "KY"))

;; ============================================================================
;; ONBOARDING ORCHESTRATION
;; ============================================================================

(onboarding.workflow
  :ob-id "CBU-2025-001"
  :current-phase "DOCUMENT_COLLECTION"
  :next-phase "COMPLIANCE_VERIFICATION"
  :dependencies [
    {:domain "kyc" :status "IN_PROGRESS"}
    {:domain "document" :status "COMPLETED"}
    {:domain "isda" :status "PENDING"}
    {:domain "compliance" :status "IN_PROGRESS"}
  ]
  :estimated-completion "2025-02-15")

(onboarding.finalize
  :ob-id "CBU-2025-001"
  :ready-when "ALL_DOMAINS_COMPLETE"
  :notification-required true
  :stakeholders ["relationship-manager" "compliance-officer" "operations-team"])
