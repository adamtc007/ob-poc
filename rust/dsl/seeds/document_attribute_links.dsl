;; Document-Attribute Links Seed File
;; This file creates mappings between document types and attributes,
;; defining which documents can provide (SOURCE) or require (SINK) which attributes.
;;
;; Direction values:
;;   SOURCE - Document provides this attribute (extraction)
;;   SINK   - Document requires this attribute (fulfillment)
;;   BOTH   - Document both provides and requires
;;
;; Extraction methods: OCR, AI, MRZ, IMAGE, MANUAL, DERIVED
;; Proof strength: PRIMARY, SECONDARY, SUPPORTING

;; ============================================================================
;; IDENTITY DOCUMENTS -> ATTRIBUTES
;; ============================================================================

;; PASSPORT
(attribute.map-to-document
    :document-type "PASSPORT"
    :attribute "attr.identity.passport_number"
    :direction "SOURCE"
    :extraction-method "MRZ"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "PASSPORT"
    :attribute "attr.identity.family_name"
    :direction "SOURCE"
    :extraction-method "MRZ"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "PASSPORT"
    :attribute "attr.identity.given_name"
    :direction "SOURCE"
    :extraction-method "MRZ"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "PASSPORT"
    :attribute "attr.identity.date_of_birth"
    :direction "SOURCE"
    :extraction-method "MRZ"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "PASSPORT"
    :attribute "attr.identity.nationality"
    :direction "SOURCE"
    :extraction-method "MRZ"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "PASSPORT"
    :attribute "attr.identity.gender"
    :direction "SOURCE"
    :extraction-method "MRZ"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "PASSPORT"
    :attribute "attr.identity.place_of_birth"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative false
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "PASSPORT"
    :attribute "attr.identity.photo"
    :direction "SOURCE"
    :extraction-method "IMAGE"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; NATIONAL_ID
(attribute.map-to-document
    :document-type "NATIONAL_ID"
    :attribute "attr.identity.full_name"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "NATIONAL_ID"
    :attribute "attr.identity.date_of_birth"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "NATIONAL_ID"
    :attribute "attr.identity.nationality"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; DRIVERS_LICENSE
(attribute.map-to-document
    :document-type "DRIVERS_LICENSE"
    :attribute "attr.identity.full_name"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "SECONDARY")

(attribute.map-to-document
    :document-type "DRIVERS_LICENSE"
    :attribute "attr.identity.date_of_birth"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "SECONDARY")

;; BIRTH_CERTIFICATE
(attribute.map-to-document
    :document-type "BIRTH_CERTIFICATE"
    :attribute "attr.identity.full_name"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "BIRTH_CERTIFICATE"
    :attribute "attr.identity.date_of_birth"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "BIRTH_CERTIFICATE"
    :attribute "attr.identity.place_of_birth"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "BIRTH_CERTIFICATE"
    :attribute "attr.identity.father_name"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative false
    :proof-strength "SUPPORTING")

(attribute.map-to-document
    :document-type "BIRTH_CERTIFICATE"
    :attribute "attr.identity.mother_name"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative false
    :proof-strength "SUPPORTING")

;; ============================================================================
;; CORPORATE DOCUMENTS -> ATTRIBUTES
;; ============================================================================

;; CERT_OF_INCORPORATION
(attribute.map-to-document
    :document-type "CERT_OF_INCORPORATION"
    :attribute "attr.identity.legal_name"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "CERT_OF_INCORPORATION"
    :attribute "attr.identity.registration_number"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "CERT_OF_INCORPORATION"
    :attribute "attr.identity.incorporation_date"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; REGISTER_OF_SHAREHOLDERS
(attribute.map-to-document
    :document-type "REGISTER_OF_SHAREHOLDERS"
    :attribute "attr.ubo.ownership_percentage"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; REGISTER_OF_DIRECTORS
(attribute.map-to-document
    :document-type "REGISTER_OF_DIRECTORS"
    :attribute "attr.identity.full_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; ============================================================================
;; TAX DOCUMENTS -> ATTRIBUTES
;; ============================================================================

;; W8_BEN (Individual)
(attribute.map-to-document
    :document-type "W8_BEN"
    :attribute "attr.identity.legal_name"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "W8_BEN"
    :attribute "attr.identity.date_of_birth"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "W8_BEN"
    :attribute "attr.tax.treaty_country"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative false
    :proof-strength "PRIMARY")

;; W8_BEN_E (Entity)
(attribute.map-to-document
    :document-type "W8_BEN_E"
    :attribute "attr.identity.legal_name"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "W8_BEN_E"
    :attribute "attr.tax.chapter3_status"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "W8_BEN_E"
    :attribute "attr.tax.chapter4_status"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "W8_BEN_E"
    :attribute "attr.tax.giin"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative false
    :proof-strength "PRIMARY")

;; W9
(attribute.map-to-document
    :document-type "W9"
    :attribute "attr.identity.legal_name"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "W9"
    :attribute "attr.tax.us_tin"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; FATCA_SELF_CERT
(attribute.map-to-document
    :document-type "FATCA_SELF_CERT"
    :attribute "attr.identity.legal_name"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "FATCA_SELF_CERT"
    :attribute "attr.tax.giin"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative false
    :proof-strength "PRIMARY")

;; CRS_SELF_CERT
(attribute.map-to-document
    :document-type "CRS_SELF_CERT"
    :attribute "attr.identity.legal_name"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; TAX_RESIDENCY_CERT
(attribute.map-to-document
    :document-type "TAX_RESIDENCY_CERT"
    :attribute "attr.identity.legal_name"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; P60 (UK Employment Tax)
(attribute.map-to-document
    :document-type "P60"
    :attribute "attr.identity.legal_name"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "P60"
    :attribute "attr.tax.ni_number"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; SA302 (UK Self Assessment)
(attribute.map-to-document
    :document-type "SA302"
    :attribute "attr.identity.legal_name"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "SA302"
    :attribute "attr.tax.utr"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; ============================================================================
;; FINANCIAL DOCUMENTS -> ATTRIBUTES
;; ============================================================================

;; BANK_STATEMENT
(attribute.map-to-document
    :document-type "BANK_STATEMENT"
    :attribute "attr.identity.full_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "BANK_STATEMENT"
    :attribute "attr.financial.bank_account_number"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "BANK_STATEMENT"
    :attribute "attr.financial.sort_code"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "BANK_STATEMENT"
    :attribute "attr.financial.iban"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "BANK_STATEMENT"
    :attribute "attr.financial.bic_swift"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "BANK_STATEMENT"
    :attribute "attr.financial.bank_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "BANK_STATEMENT"
    :attribute "attr.financial.account_currency"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; AUDITED_ACCOUNTS
(attribute.map-to-document
    :document-type "AUDITED_ACCOUNTS"
    :attribute "attr.identity.legal_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "AUDITED_ACCOUNTS"
    :attribute "attr.financial.total_assets"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "AUDITED_ACCOUNTS"
    :attribute "attr.financial.revenue"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "AUDITED_ACCOUNTS"
    :attribute "attr.financial.net_worth"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; SOURCE_OF_WEALTH
(attribute.map-to-document
    :document-type "SOURCE_OF_WEALTH"
    :attribute "attr.financial.source_of_wealth"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "SOURCE_OF_WEALTH"
    :attribute "attr.financial.net_worth"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

;; SOURCE_OF_FUNDS
(attribute.map-to-document
    :document-type "SOURCE_OF_FUNDS"
    :attribute "attr.financial.source_of_funds"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; ============================================================================
;; ADDRESS PROOF DOCUMENTS -> ATTRIBUTES
;; ============================================================================

;; UTILITY_BILL_ELECTRIC
(attribute.map-to-document
    :document-type "UTILITY_BILL_ELECTRIC"
    :attribute "attr.identity.full_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "UTILITY_BILL_ELECTRIC"
    :attribute "attr.address.service_address"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "UTILITY_BILL_ELECTRIC"
    :attribute "attr.address.statement_date"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "UTILITY_BILL_ELECTRIC"
    :attribute "attr.address.utility_account_number"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "SUPPORTING")

;; UTILITY_BILL_GAS
(attribute.map-to-document
    :document-type "UTILITY_BILL_GAS"
    :attribute "attr.identity.full_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "UTILITY_BILL_GAS"
    :attribute "attr.address.service_address"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "UTILITY_BILL_GAS"
    :attribute "attr.address.statement_date"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; UTILITY_BILL_WATER
(attribute.map-to-document
    :document-type "UTILITY_BILL_WATER"
    :attribute "attr.identity.full_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "UTILITY_BILL_WATER"
    :attribute "attr.address.service_address"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "UTILITY_BILL_WATER"
    :attribute "attr.address.statement_date"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; COUNCIL_TAX_BILL
(attribute.map-to-document
    :document-type "COUNCIL_TAX_BILL"
    :attribute "attr.identity.full_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "COUNCIL_TAX_BILL"
    :attribute "attr.address.council_tax_band"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "SUPPORTING")

(attribute.map-to-document
    :document-type "COUNCIL_TAX_BILL"
    :attribute "attr.address.council_tax_reference"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "SUPPORTING")

;; TENANCY_AGREEMENT
(attribute.map-to-document
    :document-type "TENANCY_AGREEMENT"
    :attribute "attr.identity.full_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "TENANCY_AGREEMENT"
    :attribute "attr.address.landlord_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "SUPPORTING")

(attribute.map-to-document
    :document-type "TENANCY_AGREEMENT"
    :attribute "attr.address.tenancy_start_date"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "TENANCY_AGREEMENT"
    :attribute "attr.address.tenancy_end_date"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "TENANCY_AGREEMENT"
    :attribute "attr.address.monthly_rent"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "SUPPORTING")

;; ============================================================================
;; UBO/OWNERSHIP DOCUMENTS -> ATTRIBUTES
;; ============================================================================

;; UBO_DECLARATION
(attribute.map-to-document
    :document-type "UBO_DECLARATION"
    :attribute "attr.ubo.ownership_percentage"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "UBO_DECLARATION"
    :attribute "attr.ubo.ownership_type"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "UBO_DECLARATION"
    :attribute "attr.ubo.control_type"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "UBO_DECLARATION"
    :attribute "attr.identity.date_of_birth"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "UBO_DECLARATION"
    :attribute "attr.identity.nationality"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "UBO_DECLARATION"
    :attribute "attr.ubo.is_pep"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; OWNERSHIP_CHART
(attribute.map-to-document
    :document-type "OWNERSHIP_CHART"
    :attribute "attr.ubo.ownership_percentage"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "OWNERSHIP_CHART"
    :attribute "attr.ubo.layers_to_ubo"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "OWNERSHIP_CHART"
    :attribute "attr.ubo.intermediate_entities"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

;; PSC_REGISTER
(attribute.map-to-document
    :document-type "PSC_REGISTER"
    :attribute "attr.identity.full_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "PSC_REGISTER"
    :attribute "attr.ubo.psc_nature_of_control"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "PSC_REGISTER"
    :attribute "attr.identity.date_of_birth"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "PSC_REGISTER"
    :attribute "attr.identity.nationality"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; NOMINEE_DECLARATION
(attribute.map-to-document
    :document-type "NOMINEE_DECLARATION"
    :attribute "attr.ubo.nominee_arrangement"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "NOMINEE_DECLARATION"
    :attribute "attr.ubo.nominee_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; POWER_OF_ATTORNEY
(attribute.map-to-document
    :document-type "POWER_OF_ATTORNEY"
    :attribute "attr.identity.full_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; ============================================================================
;; FUND DOCUMENTS -> ATTRIBUTES
;; ============================================================================

;; FUND_PROSPECTUS
(attribute.map-to-document
    :document-type "FUND_PROSPECTUS"
    :attribute "attr.fund.fund_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "FUND_PROSPECTUS"
    :attribute "attr.fund.fund_type"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "FUND_PROSPECTUS"
    :attribute "attr.fund.investment_strategy"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "FUND_PROSPECTUS"
    :attribute "attr.fund.base_currency"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "FUND_PROSPECTUS"
    :attribute "attr.fund.investment_manager"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "FUND_PROSPECTUS"
    :attribute "attr.fund.custodian"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "FUND_PROSPECTUS"
    :attribute "attr.fund.management_fee"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; OFFERING_MEMORANDUM (PPM)
(attribute.map-to-document
    :document-type "OFFERING_MEMORANDUM"
    :attribute "attr.fund.fund_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "OFFERING_MEMORANDUM"
    :attribute "attr.fund.investment_manager"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "OFFERING_MEMORANDUM"
    :attribute "attr.fund.management_fee"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "OFFERING_MEMORANDUM"
    :attribute "attr.fund.lock_up_period"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "OFFERING_MEMORANDUM"
    :attribute "attr.fund.minimum_investment"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; KIID
(attribute.map-to-document
    :document-type "KIID"
    :attribute "attr.fund.fund_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; ============================================================================
;; TRUST DOCUMENTS -> ATTRIBUTES
;; ============================================================================

;; TRUST_DEED
(attribute.map-to-document
    :document-type "TRUST_DEED"
    :attribute "attr.trust.trust_name"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "TRUST_DEED"
    :attribute "attr.trust.establishment_date"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "TRUST_DEED"
    :attribute "attr.trust.settlor"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "TRUST_DEED"
    :attribute "attr.trust.trustees"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "TRUST_DEED"
    :attribute "attr.trust.beneficiaries"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "TRUST_DEED"
    :attribute "attr.trust.governing_law"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "TRUST_DEED"
    :attribute "attr.trust.trust_type"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "TRUST_DEED"
    :attribute "attr.trust.is_revocable"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "TRUST_DEED"
    :attribute "attr.trust.protector"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

;; SCHEDULE_OF_BENEFICIARIES
(attribute.map-to-document
    :document-type "SCHEDULE_OF_BENEFICIARIES"
    :attribute "attr.trust.beneficiaries"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; ============================================================================
;; REGULATORY DOCUMENTS -> ATTRIBUTES
;; ============================================================================

;; LEI_CERTIFICATE
(attribute.map-to-document
    :document-type "LEI_CERTIFICATE"
    :attribute "attr.identity.legal_name"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "LEI_CERTIFICATE"
    :attribute "attr.regulatory.lei"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "LEI_CERTIFICATE"
    :attribute "attr.regulatory.lei_status"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; FCA_AUTHORIZATION
(attribute.map-to-document
    :document-type "FCA_AUTHORIZATION"
    :attribute "attr.identity.legal_name"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "FCA_AUTHORIZATION"
    :attribute "attr.regulatory.fca_firm_ref"
    :direction "SOURCE"
    :extraction-method "OCR"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "FCA_AUTHORIZATION"
    :attribute "attr.regulatory.fca_permissions"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

;; SANCTIONS_SCREENING_REPORT
(attribute.map-to-document
    :document-type "SANCTIONS_SCREENING_REPORT"
    :attribute "attr.regulatory.sanctions_match"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "SANCTIONS_SCREENING_REPORT"
    :attribute "attr.regulatory.sanctions_lists_checked"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "SANCTIONS_SCREENING_REPORT"
    :attribute "attr.regulatory.screening_date"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "SANCTIONS_SCREENING_REPORT"
    :attribute "attr.regulatory.screening_provider"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

;; PEP_SCREENING_REPORT
(attribute.map-to-document
    :document-type "PEP_SCREENING_REPORT"
    :attribute "attr.ubo.is_pep"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "PEP_SCREENING_REPORT"
    :attribute "attr.ubo.pep_category"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "PEP_SCREENING_REPORT"
    :attribute "attr.regulatory.screening_date"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "PEP_SCREENING_REPORT"
    :attribute "attr.regulatory.screening_provider"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

;; ADVERSE_MEDIA_REPORT
(attribute.map-to-document
    :document-type "ADVERSE_MEDIA_REPORT"
    :attribute "attr.regulatory.adverse_media_found"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "ADVERSE_MEDIA_REPORT"
    :attribute "attr.regulatory.adverse_media_categories"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative false
    :proof-strength "PRIMARY")

(attribute.map-to-document
    :document-type "ADVERSE_MEDIA_REPORT"
    :attribute "attr.regulatory.screening_date"
    :direction "SOURCE"
    :extraction-method "AI"
    :is-authoritative true
    :proof-strength "PRIMARY")

;; End of Document-Attribute Links Seed File
