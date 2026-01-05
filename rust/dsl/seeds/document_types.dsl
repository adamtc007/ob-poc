;; Document Types Seed File
;; This file populates the document_types catalogue with additional document types
;; for KYC/AML onboarding workflows.
;;
;; Categories covered:
;; - ADDRESS_PROOF: Utility bills, tenancy agreements, government correspondence
;; - TAX: US forms (W-8 series, 1099s, K-1), UK forms (P60, P45, SA302)
;; - UBO: Beneficial ownership declarations, ownership charts, nominee agreements
;; - REGULATORY: FCA, SEC, MiFID authorizations, screening reports
;; - CORPORATE: Additional governance documents

;; ============================================================================
;; ADDRESS PROOF DOCUMENTS
;; ============================================================================

(document-type.ensure
    :type-code "UTILITY_BILL_ELECTRIC"
    :display-name "Electricity Bill"
    :category "ADDRESS_PROOF"
    :domain "address"
    :description "Electricity utility bill for address verification. Must be dated within 3 months.")

(document-type.ensure
    :type-code "UTILITY_BILL_GAS"
    :display-name "Gas Bill"
    :category "ADDRESS_PROOF"
    :domain "address"
    :description "Gas utility bill for address verification. Must be dated within 3 months.")

(document-type.ensure
    :type-code "UTILITY_BILL_WATER"
    :display-name "Water Bill"
    :category "ADDRESS_PROOF"
    :domain "address"
    :description "Water utility bill for address verification. Must be dated within 3 months.")

(document-type.ensure
    :type-code "TELEPHONE_BILL"
    :display-name "Telephone Bill"
    :category "ADDRESS_PROOF"
    :domain "address"
    :description "Landline telephone bill for address verification. Mobile phone bills typically not accepted. Must be dated within 3 months.")

(document-type.ensure
    :type-code "COUNCIL_TAX_BILL"
    :display-name "Council Tax Bill"
    :category "ADDRESS_PROOF"
    :domain "address"
    :description "UK Council Tax bill for address verification. Valid for current tax year.")

(document-type.ensure
    :type-code "TENANCY_AGREEMENT"
    :display-name "Tenancy Agreement"
    :category "ADDRESS_PROOF"
    :domain "address"
    :description "Current rental/lease agreement showing residential address.")

(document-type.ensure
    :type-code "PROPERTY_TAX_BILL"
    :display-name "Property Tax Bill"
    :category "ADDRESS_PROOF"
    :domain "address"
    :description "Property/real estate tax bill for address verification.")

(document-type.ensure
    :type-code "BANK_LETTER_ADDRESS"
    :display-name "Bank Address Confirmation Letter"
    :category "ADDRESS_PROOF"
    :domain "address"
    :description "Letter from bank confirming account holder address. Must be dated within 3 months.")

;; ============================================================================
;; ADDITIONAL US TAX DOCUMENTS
;; ============================================================================

(document-type.ensure
    :type-code "FORM_1099_DIV"
    :display-name "Form 1099-DIV"
    :category "TAX"
    :domain "us_reporting"
    :description "Dividends and Distributions information return.")

(document-type.ensure
    :type-code "FORM_1099_INT"
    :display-name "Form 1099-INT"
    :category "TAX"
    :domain "us_reporting"
    :description "Interest Income information return.")

(document-type.ensure
    :type-code "FORM_1099_B"
    :display-name "Form 1099-B"
    :category "TAX"
    :domain "us_reporting"
    :description "Proceeds from Broker and Barter Exchange Transactions.")

(document-type.ensure
    :type-code "FORM_K1"
    :display-name "Schedule K-1"
    :category "TAX"
    :domain "us_reporting"
    :description "Partner's/Shareholder's Share of Income, Deductions, Credits (Form 1065/1120-S).")

;; ============================================================================
;; UK TAX DOCUMENTS
;; ============================================================================

(document-type.ensure
    :type-code "P60"
    :display-name "P60 End of Year Certificate"
    :category "TAX"
    :domain "uk_employment"
    :description "UK annual summary of pay and tax deducted by employer.")

(document-type.ensure
    :type-code "P45"
    :display-name "P45 Leaving Certificate"
    :category "TAX"
    :domain "uk_employment"
    :description "UK certificate given when leaving employment showing tax paid to date.")

(document-type.ensure
    :type-code "SA302"
    :display-name "SA302 Tax Calculation"
    :category "TAX"
    :domain "uk_self_assessment"
    :description "UK HMRC tax calculation from self-assessment return.")

(document-type.ensure
    :type-code "TAX_CLEARANCE_CERT"
    :display-name "Tax Clearance Certificate"
    :category "TAX"
    :domain "clearance"
    :description "Certificate confirming tax affairs are in order. Required for certain transactions.")

(document-type.ensure
    :type-code "VAT_REGISTRATION"
    :display-name "VAT Registration Certificate"
    :category "TAX"
    :domain "indirect_tax"
    :description "Certificate of VAT/GST registration.")

(document-type.ensure
    :type-code "DOUBLE_TAX_TREATY_CERT"
    :display-name "Double Taxation Treaty Certificate"
    :category "TAX"
    :domain "treaty"
    :description "Certificate of residence for double taxation treaty purposes.")

;; ============================================================================
;; UBO / BENEFICIAL OWNERSHIP DOCUMENTS
;; ============================================================================

(document-type.ensure
    :type-code "BO_REGISTER_EXTRACT"
    :display-name "Beneficial Ownership Register Extract"
    :category "UBO"
    :domain "beneficial_ownership"
    :description "Extract from national beneficial ownership register (EU AMLD / jurisdiction specific).")

(document-type.ensure
    :type-code "NOMINEE_DECLARATION"
    :display-name "Nominee Declaration"
    :category "UBO"
    :domain "beneficial_ownership"
    :description "Declaration confirming nominee arrangement and identifying the beneficial owner.")

(document-type.ensure
    :type-code "FAMILY_TREE_DIAGRAM"
    :display-name "Family Tree Diagram"
    :category "UBO"
    :domain "beneficial_ownership"
    :description "Family relationship diagram for trusts, estates, or family-controlled entities.")

(document-type.ensure
    :type-code "COURT_ORDER"
    :display-name "Court Order"
    :category "UBO"
    :domain "legal"
    :description "Court order relating to guardianship, administration, or control.")

(document-type.ensure
    :type-code "PROBATE_GRANT"
    :display-name "Grant of Probate"
    :category "UBO"
    :domain "estate"
    :description "Court document granting authority to administer a deceased person's estate.")

(document-type.ensure
    :type-code "LETTERS_OF_ADMINISTRATION"
    :display-name "Letters of Administration"
    :category "UBO"
    :domain "estate"
    :description "Court document appointing administrator for intestate estate.")

(document-type.ensure
    :type-code "WILL_TESTAMENT"
    :display-name "Last Will and Testament"
    :category "UBO"
    :domain "estate"
    :description "Deceased person's will showing beneficiaries and executors.")

;; ============================================================================
;; REGULATORY / COMPLIANCE DOCUMENTS
;; ============================================================================

(document-type.ensure
    :type-code "FCA_AUTHORIZATION"
    :display-name "FCA Authorization Letter"
    :category "REGULATORY"
    :domain "authorization"
    :description "UK Financial Conduct Authority authorization/permissions letter.")

(document-type.ensure
    :type-code "FINRA_REGISTRATION"
    :display-name "FINRA Registration"
    :category "REGULATORY"
    :domain "authorization"
    :description "US FINRA broker-dealer registration.")

(document-type.ensure
    :type-code "MIFID_AUTHORIZATION"
    :display-name "MiFID II Authorization"
    :category "REGULATORY"
    :domain "authorization"
    :description "EU Markets in Financial Instruments Directive authorization.")

(document-type.ensure
    :type-code "AIFMD_AUTHORIZATION"
    :display-name "AIFMD Authorization"
    :category "REGULATORY"
    :domain "authorization"
    :description "Alternative Investment Fund Managers Directive authorization.")

(document-type.ensure
    :type-code "UCITS_AUTHORIZATION"
    :display-name "UCITS Authorization"
    :category "REGULATORY"
    :domain "authorization"
    :description "UCITS fund authorization certificate.")

(document-type.ensure
    :type-code "AML_REGISTRATION"
    :display-name "AML Registration Certificate"
    :category "REGULATORY"
    :domain "registration"
    :description "Anti-Money Laundering supervisory registration (e.g., HMRC MSB registration).")

(document-type.ensure
    :type-code "SANCTIONS_SCREENING_REPORT"
    :display-name "Sanctions Screening Report"
    :category "REGULATORY"
    :domain "screening"
    :description "Results of sanctions list screening (OFAC, EU, UN, HMT).")

(document-type.ensure
    :type-code "PEP_SCREENING_REPORT"
    :display-name "PEP Screening Report"
    :category "REGULATORY"
    :domain "screening"
    :description "Politically Exposed Persons screening results.")

(document-type.ensure
    :type-code "ADVERSE_MEDIA_REPORT"
    :display-name "Adverse Media Report"
    :category "REGULATORY"
    :domain "screening"
    :description "Adverse/negative media screening results.")

(document-type.ensure
    :type-code "COMPLIANCE_MANUAL"
    :display-name "Compliance Manual"
    :category "REGULATORY"
    :domain "policies"
    :description "Internal compliance policies and procedures manual.")

(document-type.ensure
    :type-code "AML_RISK_ASSESSMENT"
    :display-name "AML Risk Assessment"
    :category "REGULATORY"
    :domain "risk"
    :description "Business-wide or customer-specific AML risk assessment.")

;; ============================================================================
;; CORPORATE GOVERNANCE DOCUMENTS
;; ============================================================================

(document-type.ensure
    :type-code "SHAREHOLDER_AGREEMENT"
    :display-name "Shareholder Agreement"
    :category "CORPORATE"
    :domain "governance"
    :description "Agreement between shareholders governing their relationship and rights.")

(document-type.ensure
    :type-code "DRAG_ALONG_AGREEMENT"
    :display-name "Drag-Along Agreement"
    :category "CORPORATE"
    :domain "governance"
    :description "Agreement requiring minority shareholders to join in sale.")

(document-type.ensure
    :type-code "TAG_ALONG_AGREEMENT"
    :display-name "Tag-Along Agreement"
    :category "CORPORATE"
    :domain "governance"
    :description "Agreement allowing minority shareholders to join in sale.")

(document-type.ensure
    :type-code "OPTION_AGREEMENT"
    :display-name "Stock Option Agreement"
    :category "CORPORATE"
    :domain "equity"
    :description "Agreement granting stock options.")

(document-type.ensure
    :type-code "WARRANT_AGREEMENT"
    :display-name "Warrant Agreement"
    :category "CORPORATE"
    :domain "equity"
    :description "Agreement for share purchase warrants.")

(document-type.ensure
    :type-code "CONVERTIBLE_NOTE"
    :display-name "Convertible Note Agreement"
    :category "CORPORATE"
    :domain "debt"
    :description "Convertible debt instrument agreement.")

(document-type.ensure
    :type-code "SAFE_AGREEMENT"
    :display-name "SAFE Agreement"
    :category "CORPORATE"
    :domain "equity"
    :description "Simple Agreement for Future Equity.")

(document-type.ensure
    :type-code "CERT_OF_DISSOLUTION"
    :display-name "Certificate of Dissolution"
    :category "CORPORATE"
    :domain "status"
    :description "Certificate confirming company dissolution.")

(document-type.ensure
    :type-code "CERT_OF_MERGER"
    :display-name "Certificate of Merger"
    :category "CORPORATE"
    :domain "status"
    :description "Certificate confirming merger/amalgamation.")

(document-type.ensure
    :type-code "CERT_OF_CONVERSION"
    :display-name "Certificate of Conversion"
    :category "CORPORATE"
    :domain "status"
    :description "Certificate confirming entity type conversion.")

;; ============================================================================
;; INSURANCE / SECURITY DOCUMENTS
;; ============================================================================

(document-type.ensure
    :type-code "INSURANCE_DO"
    :display-name "D&O Insurance Policy"
    :category "INSURANCE"
    :domain "coverage"
    :description "Directors and Officers liability insurance policy.")

(document-type.ensure
    :type-code "INSURANCE_EO"
    :display-name "E&O Insurance Policy"
    :category "INSURANCE"
    :domain "coverage"
    :description "Errors and Omissions / Professional Indemnity insurance policy.")

(document-type.ensure
    :type-code "GUARANTEE_AGREEMENT"
    :display-name "Guarantee Agreement"
    :category "SECURITY"
    :domain "credit_support"
    :description "Guarantee or surety agreement.")

(document-type.ensure
    :type-code "SECURITY_AGREEMENT"
    :display-name "Security Agreement"
    :category "SECURITY"
    :domain "collateral"
    :description "General security agreement granting security interest.")
