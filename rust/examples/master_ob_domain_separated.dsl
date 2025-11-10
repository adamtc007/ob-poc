;; Master OB Request with Domain-Separated DSLs
;; OB Request ID: CBU-2025-001
;; This demonstrates how domain DSLs are stored separately but linked to the master OB request

;; ============================================================================
;; KYC DOMAIN DSL (domain_name="kyc", business_reference="CBU-2025-001")
;; ============================================================================

;; Entity KYC verification for the master onboarding request
(kyc.verify
  :entity-id "zenith-capital-lp"
  :verification-level "ENHANCED"
  :risk-rating "MEDIUM"
  :documents ["certificate-incorporation" "memorandum-association" "partnership-agreement"]
  :jurisdictions ["KY" "US"]
  :beneficial-owners ["john-smith" "mary-jones"])

;; Risk assessment linked to master OB
(kyc.assess_risk
  :entity-id "zenith-capital-lp"
  :risk-level "MEDIUM"
  :pep-screening "PASSED"
  :sanctions-screening "PASSED"
  :aml-checks "COMPLETED"
  :source-of-funds "INSTITUTIONAL")

;; Document collection for KYC
(kyc.collect_document
  :entity-id "zenith-capital-lp"
  :document-type "PASSPORT"
  :document-id "passport-john-smith-001"
  :verification-status "VERIFIED")

(kyc.collect_document
  :entity-id "zenith-capital-lp"
  :document-type "UTILITY_BILL"
  :document-id "utility-mary-jones-001"
  :verification-status "PENDING")

;; Sanctions screening
(kyc.screen_sanctions
  :entity-id "zenith-capital-lp"
  :screening-provider "WORLD_CHECK"
  :screening-result "NO_MATCHES"
  :screening-date "2025-01-15")

;; PEP check
(kyc.check_pep
  :entity-id "zenith-capital-lp"
  :pep-result "NOT_PEP"
  :checked-individuals ["john-smith" "mary-jones"])

;; Address validation
(kyc.validate_address
  :entity-id "zenith-capital-lp"
  :address "123 Cayman Financial Centre, Grand Cayman, KY"
  :validation-method "POSTAL_SERVICE"
  :validation-result "CONFIRMED")

;; ============================================================================
;; DOCUMENT DOMAIN DSL (domain_name="document", business_reference="CBU-2025-001")
;; ============================================================================

;; Incorporation documents for the master onboarding
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

(document.verify
  :document-id "doc-partnership-agreement"
  :verification-method "NOTARIZATION"
  :verification-result "AUTHENTIC"
  :verified-by "cayman-notary-public")

;; Extract key data for onboarding
(document.extract
  :document-id "doc-partnership-agreement"
  :extraction-target "ENTITY_DETAILS"
  :extracted-data {
    :legal-name "Zenith Capital Partners LP"
    :registration-number "KY-123456-LP"
    :registered-address "Grand Cayman, KY"
    :general-partner "Zenith GP Ltd"
  })

;; Link documents to entities
(document.link
  :document-id "doc-incorporation-ky-001"
  :linked-entity "zenith-capital-lp"
  :relationship-type "INCORPORATION_CERTIFICATE")

(document.link
  :document-id "doc-partnership-agreement"
  :linked-entity "zenith-capital-lp"
  :relationship-type "GOVERNING_DOCUMENT")

;; ============================================================================
;; ISDA DOMAIN DSL (domain_name="isda", business_reference="CBU-2025-001")
;; ============================================================================

;; Establish ISDA Master Agreement for the onboarding entity
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
  :maturity-date "2025-07-20")

;; Portfolio valuation
(isda.value_portfolio
  :agreement-id "ISDA-ZENITH-GS-001"
  :valuation-date "2025-01-20"
  :portfolio-value 10500000.0
  :currency "USD"
  :valuation-method "MARK_TO_MARKET")

;; ============================================================================
;; COMPLIANCE DOMAIN DSL (domain_name="compliance", business_reference="CBU-2025-001")
;; ============================================================================

;; FATCA compliance check
(compliance.fatca_check
  :entity-id "zenith-capital-lp"
  :classification "NON_US_ENTITY"
  :giin "ABC123.KY.456"
  :reporting-required true
  :reporting-jurisdiction "KY")

;; CRS compliance
(compliance.crs_check
  :entity-id "zenith-capital-lp"
  :reporting-jurisdiction "KY"
  :account-holder-type "PASSIVE_NFE"
  :controlling-persons ["john-smith" "mary-jones"])

;; AML compliance check
(compliance.aml_check
  :entity-id "zenith-capital-lp"
  :aml-program-adequate true
  :suspicious-activity-monitoring "ACTIVE"
  :customer-due-diligence "ENHANCED"
  :ongoing-monitoring "QUARTERLY")

;; Final compliance verification
(compliance.verify
  :entity-id "zenith-capital-lp"
  :compliance-status "COMPLIANT"
  :regulatory-clearance ["CFTC" "SEC"]
  :exemptions-claimed ["3C1_PRIVATE_FUND"]
  :next-review-date "2025-07-15")

;; ============================================================================
;; CASE MANAGEMENT DSL (domain_name="case", business_reference="CBU-2025-001")
;; ============================================================================

;; Create the master onboarding case
(case.create
  :case-id "CBU-2025-001"
  :case-type "HEDGE_FUND_ONBOARDING"
  :entity-name "Zenith Capital Partners LP"
  :jurisdiction "KY"
  :priority "HIGH"
  :assigned-to "relationship-manager-001"
  :estimated-completion "2025-02-15")

;; Update case with progress
(case.update
  :case-id "CBU-2025-001"
  :status "IN_PROGRESS"
  :progress-notes "KYC documentation received, ISDA negotiations initiated"
  :completion-percentage 60
  :next-action "Compliance review"
  :updated-by "operations-team-002")

;; ============================================================================
;; ENTITY GRAPH DSL (domain_name="graph", business_reference="CBU-2025-001")
;; ============================================================================

;; Define the main hedge fund entity
(entity
  :id "zenith-capital-lp"
  :label "Company"
  :props {
    :legal-name "Zenith Capital Partners LP"
    :jurisdiction "KY"
    :entity-type "LIMITED_PARTNERSHIP"
    :incorporation-date "2020-01-15"
    :registration-number "KY-123456-LP"
    :aum 750000000.0
    :investment-strategy "LONG_SHORT_EQUITY"
    :prime-broker "goldman-sachs"
    :administrator "ssc-technologies"
  })

;; Define the general partner
(entity
  :id "zenith-gp-ltd"
  :label "Company"
  :props {
    :legal-name "Zenith GP Ltd"
    :jurisdiction "KY"
    :entity-type "LIMITED_COMPANY"
    :role "GENERAL_PARTNER"
  })

;; Create ownership relationship
(edge
  :from "zenith-gp-ltd"
  :to "zenith-capital-lp"
  :type "GENERAL_PARTNER"
  :props {
    :ownership-percentage 1.0
    :management-control true
    :liability "UNLIMITED"
  })
