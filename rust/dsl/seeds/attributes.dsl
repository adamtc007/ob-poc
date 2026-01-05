;; Attributes Seed File
;; This file populates the attribute_registry with additional attributes
;; for KYC/AML onboarding workflows.
;;
;; Categories covered:
;; - address: Utility/tenancy-specific address attributes
;; - tax: US W-8 form fields, UK tax forms (P60, P45, SA302)
;; - ubo: Additional beneficial ownership attributes
;; - compliance: Regulatory/screening attributes

;; ============================================================================
;; ADDRESS ATTRIBUTES (for utility bills, tenancy agreements)
;; ============================================================================

(attribute.define
    :id "attr.address.utility_account_number"
    :display-name "Utility Account Number"
    :category "address"
    :value-type "string"
    :domain "address_proof")

(attribute.define
    :id "attr.address.statement_date"
    :display-name "Statement Date"
    :category "address"
    :value-type "date"
    :domain "address_proof")

(attribute.define
    :id "attr.address.billing_period_start"
    :display-name "Billing Period Start"
    :category "address"
    :value-type "date"
    :domain "address_proof")

(attribute.define
    :id "attr.address.billing_period_end"
    :display-name "Billing Period End"
    :category "address"
    :value-type "date"
    :domain "address_proof")

(attribute.define
    :id "attr.address.service_address"
    :display-name "Service Address"
    :category "address"
    :value-type "address"
    :domain "address_proof")

(attribute.define
    :id "attr.address.landlord_name"
    :display-name "Landlord Name"
    :category "address"
    :value-type "string"
    :domain "tenancy")

(attribute.define
    :id "attr.address.tenancy_start_date"
    :display-name "Tenancy Start Date"
    :category "address"
    :value-type "date"
    :domain "tenancy")

(attribute.define
    :id "attr.address.tenancy_end_date"
    :display-name "Tenancy End Date"
    :category "address"
    :value-type "date"
    :domain "tenancy")

(attribute.define
    :id "attr.address.monthly_rent"
    :display-name "Monthly Rent"
    :category "address"
    :value-type "currency"
    :domain "tenancy")

(attribute.define
    :id "attr.address.council_tax_band"
    :display-name "Council Tax Band"
    :category "address"
    :value-type "string"
    :domain "uk_council_tax")

(attribute.define
    :id "attr.address.council_tax_reference"
    :display-name "Council Tax Reference"
    :category "address"
    :value-type "string"
    :domain "uk_council_tax")

;; ============================================================================
;; TAX ATTRIBUTES (W-8 forms, UK tax)
;; ============================================================================

;; US W-8 Form Attributes
(attribute.define
    :id "attr.tax.chapter3_status"
    :display-name "Chapter 3 Status"
    :category "tax"
    :value-type "string"
    :domain "us_withholding")

(attribute.define
    :id "attr.tax.chapter4_status"
    :display-name "Chapter 4 Status (FATCA)"
    :category "tax"
    :value-type "string"
    :domain "us_withholding")

(attribute.define
    :id "attr.tax.treaty_country"
    :display-name "Treaty Country"
    :category "tax"
    :value-type "string"
    :domain "treaty")

(attribute.define
    :id "attr.tax.treaty_article"
    :display-name "Treaty Article"
    :category "tax"
    :value-type "string"
    :domain "treaty")

(attribute.define
    :id "attr.tax.treaty_withholding_rate"
    :display-name "Treaty Withholding Rate"
    :category "tax"
    :value-type "percentage"
    :domain "treaty")

(attribute.define
    :id "attr.tax.lob_provision"
    :display-name "LOB Provision Met"
    :category "tax"
    :value-type "string"
    :domain "treaty")

(attribute.define
    :id "attr.tax.us_tin"
    :display-name "US Tax ID (EIN/SSN/ITIN)"
    :category "tax"
    :value-type "string"
    :domain "us_tax")

(attribute.define
    :id "attr.tax.ftin"
    :display-name "Foreign Tax ID Number"
    :category "tax"
    :value-type "string"
    :domain "foreign_tax")

(attribute.define
    :id "attr.tax.effectively_connected"
    :display-name "Effectively Connected Income"
    :category "tax"
    :value-type "boolean"
    :domain "us_withholding")

(attribute.define
    :id "attr.tax.us_branch"
    :display-name "US Branch"
    :category "tax"
    :value-type "boolean"
    :domain "us_withholding")

;; UK Tax Attributes
(attribute.define
    :id "attr.tax.ni_number"
    :display-name "National Insurance Number"
    :category "tax"
    :value-type "string"
    :domain "uk_tax")

(attribute.define
    :id "attr.tax.paye_reference"
    :display-name "PAYE Reference"
    :category "tax"
    :value-type "string"
    :domain "uk_tax")

(attribute.define
    :id "attr.tax.utr"
    :display-name "Unique Taxpayer Reference"
    :category "tax"
    :value-type "string"
    :domain "uk_tax")

(attribute.define
    :id "attr.tax.tax_code"
    :display-name "Tax Code"
    :category "tax"
    :value-type "string"
    :domain "uk_tax")

(attribute.define
    :id "attr.tax.gross_pay_ytd"
    :display-name "Gross Pay Year to Date"
    :category "tax"
    :value-type "currency"
    :domain "employment")

(attribute.define
    :id "attr.tax.tax_deducted_ytd"
    :display-name "Tax Deducted Year to Date"
    :category "tax"
    :value-type "currency"
    :domain "employment")

(attribute.define
    :id "attr.tax.ni_contributions_ytd"
    :display-name "NI Contributions Year to Date"
    :category "tax"
    :value-type "currency"
    :domain "uk_tax")

;; K-1 / Partnership Tax Attributes
(attribute.define
    :id "attr.tax.partner_share_income"
    :display-name "Partner Share of Income"
    :category "tax"
    :value-type "currency"
    :domain "partnership_tax")

(attribute.define
    :id "attr.tax.partner_share_loss"
    :display-name "Partner Share of Loss"
    :category "tax"
    :value-type "currency"
    :domain "partnership_tax")

(attribute.define
    :id "attr.tax.partner_capital_account"
    :display-name "Partner Capital Account"
    :category "tax"
    :value-type "currency"
    :domain "partnership_tax")

(attribute.define
    :id "attr.tax.partner_ownership_pct"
    :display-name "Partner Ownership Percentage"
    :category "tax"
    :value-type "percentage"
    :domain "partnership_tax")

;; ============================================================================
;; UBO ATTRIBUTES (beneficial ownership)
;; ============================================================================

(attribute.define
    :id "attr.ubo.ownership_type"
    :display-name "Ownership Type"
    :category "ubo"
    :value-type "string"
    :domain "beneficial_ownership")

(attribute.define
    :id "attr.ubo.is_pep"
    :display-name "Is Politically Exposed Person"
    :category "ubo"
    :value-type "boolean"
    :domain "pep")

(attribute.define
    :id "attr.ubo.pep_category"
    :display-name "PEP Category"
    :category "ubo"
    :value-type "string"
    :domain "pep")

(attribute.define
    :id "attr.ubo.layers_to_ubo"
    :display-name "Layers to UBO"
    :category "ubo"
    :value-type "integer"
    :domain "beneficial_ownership")

(attribute.define
    :id "attr.ubo.intermediate_entities"
    :display-name "Intermediate Entities"
    :category "ubo"
    :value-type "json"
    :domain "beneficial_ownership")

(attribute.define
    :id "attr.ubo.nominee_arrangement"
    :display-name "Nominee Arrangement"
    :category "ubo"
    :value-type "boolean"
    :domain "beneficial_ownership")

(attribute.define
    :id "attr.ubo.nominee_name"
    :display-name "Nominee Name"
    :category "ubo"
    :value-type "string"
    :domain "beneficial_ownership")

(attribute.define
    :id "attr.ubo.psc_nature_of_control"
    :display-name "PSC Nature of Control"
    :category "ubo"
    :value-type "json"
    :domain "uk_psc")

;; ============================================================================
;; REGULATORY / COMPLIANCE ATTRIBUTES
;; ============================================================================

(attribute.define
    :id "attr.regulatory.fca_firm_ref"
    :display-name "FCA Firm Reference Number"
    :category "compliance"
    :value-type "string"
    :domain "uk_regulatory")

(attribute.define
    :id "attr.regulatory.fca_permissions"
    :display-name "FCA Permissions"
    :category "compliance"
    :value-type "json"
    :domain "uk_regulatory")

(attribute.define
    :id "attr.regulatory.sec_file_number"
    :display-name "SEC File Number"
    :category "compliance"
    :value-type "string"
    :domain "us_regulatory")

(attribute.define
    :id "attr.regulatory.crd_number"
    :display-name "CRD Number"
    :category "compliance"
    :value-type "string"
    :domain "us_regulatory")

(attribute.define
    :id "attr.regulatory.lei_status"
    :display-name "LEI Status"
    :category "compliance"
    :value-type "string"
    :domain "entity_id")

(attribute.define
    :id "attr.regulatory.sanctions_match"
    :display-name "Sanctions Match Found"
    :category "compliance"
    :value-type "boolean"
    :domain "screening")

(attribute.define
    :id "attr.regulatory.screening_date"
    :display-name "Screening Date"
    :category "compliance"
    :value-type "date"
    :domain "screening")

(attribute.define
    :id "attr.regulatory.screening_provider"
    :display-name "Screening Provider"
    :category "compliance"
    :value-type "string"
    :domain "screening")

(attribute.define
    :id "attr.regulatory.adverse_media_found"
    :display-name "Adverse Media Found"
    :category "compliance"
    :value-type "boolean"
    :domain "screening")

(attribute.define
    :id "attr.regulatory.adverse_media_categories"
    :display-name "Adverse Media Categories"
    :category "compliance"
    :value-type "json"
    :domain "screening")
