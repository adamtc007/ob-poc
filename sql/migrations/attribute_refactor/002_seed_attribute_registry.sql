-- Attribute Dictionary Refactoring - Phase 1: Foundation
-- Migration: 002_seed_attribute_registry.sql
-- Purpose: Populate attribute registry with KYC attributes
-- Date: 2025-11-14
-- Version: 1.0
--
-- This file populates the attribute_registry table with all the typed attributes
-- defined in rust/src/domains/attributes/kyc.rs
--
-- Each INSERT corresponds to a Rust attribute definition using the macro system.

BEGIN;

-- ============================================================================
-- IDENTITY ATTRIBUTES
-- ============================================================================

INSERT INTO "ob-poc".attribute_registry (id, display_name, category, value_type, validation_rules, metadata) VALUES
(
    'attr.identity.legal_name',
    'Legal Entity Name',
    'identity',
    'string',
    '{"required": true, "min_length": 1, "max_length": 255, "pattern": "^[A-Za-z0-9\\s\\-\\.,&''()]+$"}'::jsonb,
    '{"rust_type": "LegalEntityName", "domain": "kyc"}'::jsonb
),
(
    'attr.identity.first_name',
    'First Name',
    'identity',
    'string',
    '{"required": true, "min_length": 1, "max_length": 100, "pattern": "^[A-Za-z\\s\\-'']+$"}'::jsonb,
    '{"rust_type": "FirstName", "domain": "kyc"}'::jsonb
),
(
    'attr.identity.last_name',
    'Last Name',
    'identity',
    'string',
    '{"required": true, "min_length": 1, "max_length": 100, "pattern": "^[A-Za-z\\s\\-'']+$"}'::jsonb,
    '{"rust_type": "LastName", "domain": "kyc"}'::jsonb
),
(
    'attr.identity.date_of_birth',
    'Date of Birth',
    'identity',
    'date',
    '{"required": true, "pattern": "^\\d{4}-\\d{2}-\\d{2}$"}'::jsonb,
    '{"rust_type": "DateOfBirth", "domain": "kyc"}'::jsonb
),
(
    'attr.identity.nationality',
    'Nationality',
    'identity',
    'string',
    '{"required": true, "min_length": 2, "max_length": 2, "pattern": "^[A-Z]{2}$"}'::jsonb,
    '{"rust_type": "Nationality", "domain": "kyc", "format": "ISO-3166-1 alpha-2"}'::jsonb
),
(
    'attr.identity.passport_number',
    'Passport Number',
    'identity',
    'string',
    '{"min_length": 6, "max_length": 20, "pattern": "^[A-Z0-9]+$"}'::jsonb,
    '{"rust_type": "PassportNumber", "domain": "kyc"}'::jsonb
),
(
    'attr.identity.registration_number',
    'Company Registration Number',
    'identity',
    'string',
    '{"required": true, "min_length": 1, "max_length": 50}'::jsonb,
    '{"rust_type": "RegistrationNumber", "domain": "kyc"}'::jsonb
),
(
    'attr.identity.incorporation_date',
    'Date of Incorporation',
    'identity',
    'date',
    '{"required": true, "pattern": "^\\d{4}-\\d{2}-\\d{2}$"}'::jsonb,
    '{"rust_type": "IncorporationDate", "domain": "kyc"}'::jsonb
);

-- ============================================================================
-- ENTITY ATTRIBUTES
-- ============================================================================

INSERT INTO "ob-poc".attribute_registry (id, display_name, category, value_type, validation_rules, metadata) VALUES
(
    'attr.entity.type',
    'Entity Type',
    'entity',
    'string',
    '{"required": true, "allowed_values": ["PROPER_PERSON", "CORPORATE", "TRUST", "PARTNERSHIP", "FUND"]}'::jsonb,
    '{"rust_type": "EntityType", "domain": "kyc"}'::jsonb
),
(
    'attr.entity.domicile',
    'Entity Domicile',
    'entity',
    'string',
    '{"required": true, "min_length": 2, "max_length": 2, "pattern": "^[A-Z]{2}$"}'::jsonb,
    '{"rust_type": "EntityDomicile", "domain": "kyc", "format": "ISO-3166-1 alpha-2"}'::jsonb
);

-- ============================================================================
-- FINANCIAL ATTRIBUTES (KYC Proper Person)
-- ============================================================================

INSERT INTO "ob-poc".attribute_registry (id, display_name, category, value_type, validation_rules, metadata) VALUES
(
    'attr.kyc.proper_person.net_worth',
    'Net Worth',
    'financial',
    'number',
    '{"required": true, "min_value": 0.0}'::jsonb,
    '{"rust_type": "ProperPersonNetWorth", "domain": "kyc", "subcategory": "proper_person"}'::jsonb
),
(
    'attr.kyc.proper_person.annual_income',
    'Annual Income',
    'financial',
    'number',
    '{"required": true, "min_value": 0.0}'::jsonb,
    '{"rust_type": "ProperPersonAnnualIncome", "domain": "kyc", "subcategory": "proper_person"}'::jsonb
);

-- ============================================================================
-- COMPLIANCE ATTRIBUTES
-- ============================================================================

INSERT INTO "ob-poc".attribute_registry (id, display_name, category, value_type, validation_rules, metadata) VALUES
(
    'attr.kyc.proper_person.source_of_wealth',
    'Source of Wealth',
    'compliance',
    'string',
    '{"required": true, "min_length": 10, "max_length": 500}'::jsonb,
    '{"rust_type": "SourceOfWealth", "domain": "kyc"}'::jsonb
),
(
    'attr.kyc.proper_person.source_of_funds',
    'Source of Funds',
    'compliance',
    'string',
    '{"required": true, "min_length": 10, "max_length": 500}'::jsonb,
    '{"rust_type": "SourceOfFunds", "domain": "kyc"}'::jsonb
),
(
    'attr.kyc.corporate.business_activity',
    'Primary Business Activity',
    'compliance',
    'string',
    '{"required": true, "min_length": 10, "max_length": 500}'::jsonb,
    '{"rust_type": "BusinessActivity", "domain": "kyc", "subcategory": "corporate"}'::jsonb
),
(
    'attr.kyc.corporate.regulatory_status',
    'Regulatory Status',
    'compliance',
    'string',
    '{"required": true, "min_length": 5, "max_length": 200}'::jsonb,
    '{"rust_type": "RegulatoryStatus", "domain": "kyc", "subcategory": "corporate"}'::jsonb
),
(
    'attr.compliance.fatca_status',
    'FATCA Status',
    'compliance',
    'string',
    '{"required": true, "allowed_values": ["COMPLIANT", "NON_COMPLIANT", "EXEMPT"]}'::jsonb,
    '{"rust_type": "FatcaStatus", "domain": "kyc"}'::jsonb
),
(
    'attr.compliance.crs_status',
    'CRS Status',
    'compliance',
    'string',
    '{"required": true, "allowed_values": ["COMPLIANT", "NON_COMPLIANT", "EXEMPT"]}'::jsonb,
    '{"rust_type": "CrsStatus", "domain": "kyc"}'::jsonb
),
(
    'attr.compliance.aml_status',
    'AML Status',
    'compliance',
    'string',
    '{"allowed_values": ["PASSED", "FAILED", "PENDING", "REQUIRES_REVIEW"]}'::jsonb,
    '{"rust_type": "AmlStatus", "domain": "kyc"}'::jsonb
),
(
    'attr.compliance.sanctions_check',
    'Sanctions Screening Result',
    'compliance',
    'string',
    '{"allowed_values": ["CLEAR", "HIT", "PENDING"]}'::jsonb,
    '{"rust_type": "SanctionsCheck", "domain": "kyc"}'::jsonb
);

-- ============================================================================
-- EMPLOYMENT ATTRIBUTES
-- ============================================================================

INSERT INTO "ob-poc".attribute_registry (id, display_name, category, value_type, validation_rules, metadata) VALUES
(
    'attr.kyc.proper_person.occupation',
    'Occupation',
    'employment',
    'string',
    '{"required": true, "min_length": 2, "max_length": 100}'::jsonb,
    '{"rust_type": "Occupation", "domain": "kyc"}'::jsonb
),
(
    'attr.kyc.corporate.employees_count',
    'Number of Employees',
    'employment',
    'integer',
    '{"min_value": 0}'::jsonb,
    '{"rust_type": "EmployeeCount", "domain": "kyc"}'::jsonb
);

-- ============================================================================
-- CONTACT ATTRIBUTES
-- ============================================================================

INSERT INTO "ob-poc".attribute_registry (id, display_name, category, value_type, validation_rules, metadata) VALUES
(
    'attr.contact.email',
    'Email Address',
    'contact',
    'email',
    '{"required": true, "pattern": "^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$"}'::jsonb,
    '{"rust_type": "Email", "domain": "kyc"}'::jsonb
),
(
    'attr.contact.phone',
    'Phone Number',
    'contact',
    'phone',
    '{"required": true, "pattern": "^\\+?[1-9]\\d{1,14}$"}'::jsonb,
    '{"rust_type": "PhoneNumber", "domain": "kyc", "format": "E.164"}'::jsonb
);

-- ============================================================================
-- ADDRESS ATTRIBUTES
-- ============================================================================

INSERT INTO "ob-poc".attribute_registry (id, display_name, category, value_type, validation_rules, metadata) VALUES
(
    'attr.contact.address_line1',
    'Address Line 1',
    'address',
    'string',
    '{"required": true, "min_length": 5, "max_length": 200}'::jsonb,
    '{"rust_type": "AddressLine1", "domain": "kyc"}'::jsonb
),
(
    'attr.contact.address_line2',
    'Address Line 2',
    'address',
    'string',
    '{"max_length": 200}'::jsonb,
    '{"rust_type": "AddressLine2", "domain": "kyc"}'::jsonb
),
(
    'attr.contact.city',
    'City',
    'address',
    'string',
    '{"required": true, "min_length": 2, "max_length": 100}'::jsonb,
    '{"rust_type": "City", "domain": "kyc"}'::jsonb
),
(
    'attr.contact.postal_code',
    'Postal Code',
    'address',
    'string',
    '{"required": true, "min_length": 3, "max_length": 20}'::jsonb,
    '{"rust_type": "PostalCode", "domain": "kyc"}'::jsonb
),
(
    'attr.contact.country',
    'Country',
    'address',
    'string',
    '{"required": true, "min_length": 2, "max_length": 2, "pattern": "^[A-Z]{2}$"}'::jsonb,
    '{"rust_type": "Country", "domain": "kyc", "format": "ISO-3166-1 alpha-2"}'::jsonb
);

-- ============================================================================
-- TAX ATTRIBUTES
-- ============================================================================

INSERT INTO "ob-poc".attribute_registry (id, display_name, category, value_type, validation_rules, metadata) VALUES
(
    'attr.tax.tin',
    'Tax Identification Number',
    'tax',
    'tax_id',
    '{"required": true, "min_length": 5, "max_length": 50}'::jsonb,
    '{"rust_type": "TaxIdentificationNumber", "domain": "kyc"}'::jsonb
),
(
    'attr.tax.jurisdiction',
    'Tax Jurisdiction',
    'tax',
    'string',
    '{"required": true, "min_length": 2, "max_length": 2, "pattern": "^[A-Z]{2}$"}'::jsonb,
    '{"rust_type": "TaxJurisdiction", "domain": "kyc", "format": "ISO-3166-1 alpha-2"}'::jsonb
),
(
    'attr.tax.treaty_benefits',
    'Tax Treaty Benefits Eligibility',
    'tax',
    'boolean',
    '{}'::jsonb,
    '{"rust_type": "TaxTreatyBenefits", "domain": "kyc"}'::jsonb
),
(
    'attr.tax.withholding_rate',
    'Withholding Tax Rate',
    'tax',
    'percentage',
    '{"min_value": 0.0, "max_value": 100.0}'::jsonb,
    '{"rust_type": "WithholdingRate", "domain": "kyc"}'::jsonb
);

-- ============================================================================
-- UBO (ULTIMATE BENEFICIAL OWNER) ATTRIBUTES
-- ============================================================================

INSERT INTO "ob-poc".attribute_registry (id, display_name, category, value_type, validation_rules, metadata) VALUES
(
    'attr.ubo.ownership_percentage',
    'Ownership Percentage',
    'ubo',
    'percentage',
    '{"required": true, "min_value": 0.0, "max_value": 100.0}'::jsonb,
    '{"rust_type": "UboOwnershipPercentage", "domain": "kyc"}'::jsonb
),
(
    'attr.ubo.control_type',
    'Control Type',
    'ubo',
    'string',
    '{"required": true, "allowed_values": ["DIRECT", "INDIRECT", "VOTING_RIGHTS", "OTHER"]}'::jsonb,
    '{"rust_type": "UboControlType", "domain": "kyc"}'::jsonb
),
(
    'attr.ubo.full_name',
    'UBO Full Name',
    'ubo',
    'string',
    '{"required": true, "min_length": 2, "max_length": 200}'::jsonb,
    '{"rust_type": "UboFullName", "domain": "kyc"}'::jsonb
),
(
    'attr.ubo.date_of_birth',
    'UBO Date of Birth',
    'ubo',
    'date',
    '{"required": true, "pattern": "^\\d{4}-\\d{2}-\\d{2}$"}'::jsonb,
    '{"rust_type": "UboDateOfBirth", "domain": "kyc"}'::jsonb
),
(
    'attr.ubo.nationality',
    'UBO Nationality',
    'ubo',
    'string',
    '{"required": true, "min_length": 2, "max_length": 2, "pattern": "^[A-Z]{2}$"}'::jsonb,
    '{"rust_type": "UboNationality", "domain": "kyc", "format": "ISO-3166-1 alpha-2"}'::jsonb
);

-- ============================================================================
-- RISK ATTRIBUTES
-- ============================================================================

INSERT INTO "ob-poc".attribute_registry (id, display_name, category, value_type, validation_rules, metadata) VALUES
(
    'attr.risk.profile',
    'Risk Profile',
    'risk',
    'string',
    '{"allowed_values": ["CONSERVATIVE", "MODERATE", "AGGRESSIVE", "SPECULATIVE"]}'::jsonb,
    '{"rust_type": "RiskProfile", "domain": "kyc"}'::jsonb
),
(
    'attr.risk.tolerance',
    'Risk Tolerance Score',
    'risk',
    'integer',
    '{"min_value": 1, "max_value": 10}'::jsonb,
    '{"rust_type": "RiskTolerance", "domain": "kyc"}'::jsonb
),
(
    'attr.risk.investment_experience',
    'Years of Investment Experience',
    'risk',
    'integer',
    '{"required": true, "min_value": 0}'::jsonb,
    '{"rust_type": "InvestmentExperience", "domain": "kyc"}'::jsonb
),
(
    'attr.risk.previous_losses',
    'Previous Investment Losses (%)',
    'risk',
    'percentage',
    '{"min_value": 0.0, "max_value": 100.0}'::jsonb,
    '{"rust_type": "PreviousLosses", "domain": "kyc"}'::jsonb
);

-- ============================================================================
-- BANKING/FINANCIAL ATTRIBUTES
-- ============================================================================

INSERT INTO "ob-poc".attribute_registry (id, display_name, category, value_type, validation_rules, metadata) VALUES
(
    'attr.banking.account_number',
    'Bank Account Number',
    'financial',
    'string',
    '{"required": true, "min_length": 8, "max_length": 34}'::jsonb,
    '{"rust_type": "BankAccountNumber", "domain": "kyc"}'::jsonb
),
(
    'attr.banking.iban',
    'IBAN',
    'financial',
    'string',
    '{"min_length": 15, "max_length": 34, "pattern": "^[A-Z]{2}[0-9]{2}[A-Z0-9]+$"}'::jsonb,
    '{"rust_type": "Iban", "domain": "kyc", "format": "ISO 13616"}'::jsonb
),
(
    'attr.banking.swift_code',
    'SWIFT/BIC Code',
    'financial',
    'string',
    '{"required": true, "min_length": 8, "max_length": 11, "pattern": "^[A-Z]{6}[A-Z0-9]{2}([A-Z0-9]{3})?$"}'::jsonb,
    '{"rust_type": "SwiftCode", "domain": "kyc", "format": "ISO 9362"}'::jsonb
),
(
    'attr.banking.bank_name',
    'Bank Name',
    'financial',
    'string',
    '{"required": true, "min_length": 2, "max_length": 200}'::jsonb,
    '{"rust_type": "BankName", "domain": "kyc"}'::jsonb
),
(
    'attr.kyc.corporate.aum',
    'Assets Under Management',
    'financial',
    'number',
    '{"min_value": 0.0}'::jsonb,
    '{"rust_type": "AssetsUnderManagement", "domain": "kyc", "subcategory": "corporate"}'::jsonb
),
(
    'attr.investment.subscription_amount',
    'Subscription Amount',
    'financial',
    'currency',
    '{"required": true, "min_value": 0.0}'::jsonb,
    '{"rust_type": "SubscriptionAmount", "domain": "kyc"}'::jsonb
),
(
    'attr.investment.subscription_currency',
    'Subscription Currency',
    'financial',
    'string',
    '{"required": true, "min_length": 3, "max_length": 3, "pattern": "^[A-Z]{3}$"}'::jsonb,
    '{"rust_type": "SubscriptionCurrency", "domain": "kyc", "format": "ISO-4217"}'::jsonb
),
(
    'attr.investment.subscription_date',
    'Subscription Date',
    'financial',
    'date',
    '{"required": true, "pattern": "^\\d{4}-\\d{2}-\\d{2}$"}'::jsonb,
    '{"rust_type": "SubscriptionDate", "domain": "kyc"}'::jsonb
);

-- ============================================================================
-- PRODUCT ATTRIBUTES
-- ============================================================================

INSERT INTO "ob-poc".attribute_registry (id, display_name, category, value_type, validation_rules, metadata) VALUES
(
    'attr.investment.redemption_notice_period',
    'Redemption Notice Period (days)',
    'product',
    'integer',
    '{"required": true, "min_value": 0}'::jsonb,
    '{"rust_type": "RedemptionNoticePeriod", "domain": "kyc"}'::jsonb
),
(
    'attr.fund.name',
    'Fund Name',
    'product',
    'string',
    '{"required": true, "min_length": 2, "max_length": 255}'::jsonb,
    '{"rust_type": "FundName", "domain": "kyc"}'::jsonb
),
(
    'attr.fund.strategy',
    'Investment Strategy',
    'product',
    'string',
    '{"required": true, "min_length": 10, "max_length": 500}'::jsonb,
    '{"rust_type": "FundStrategy", "domain": "kyc"}'::jsonb
),
(
    'attr.fund.base_currency',
    'Fund Base Currency',
    'product',
    'string',
    '{"required": true, "min_length": 3, "max_length": 3, "pattern": "^[A-Z]{3}$"}'::jsonb,
    '{"rust_type": "FundBaseCurrency", "domain": "kyc", "format": "ISO-4217"}'::jsonb
),
(
    'attr.fund.minimum_investment',
    'Minimum Investment Amount',
    'product',
    'currency',
    '{"required": true, "min_value": 0.0}'::jsonb,
    '{"rust_type": "MinimumInvestment", "domain": "kyc"}'::jsonb
),
(
    'attr.fund.management_fee',
    'Management Fee (%)',
    'product',
    'percentage',
    '{"required": true, "min_value": 0.0, "max_value": 100.0}'::jsonb,
    '{"rust_type": "ManagementFee", "domain": "kyc"}'::jsonb
),
(
    'attr.hedge_fund.performance_fee',
    'Performance Fee (%)',
    'product',
    'percentage',
    '{"required": true, "min_value": 0.0, "max_value": 100.0}'::jsonb,
    '{"rust_type": "PerformanceFee", "domain": "kyc"}'::jsonb
),
(
    'attr.hedge_fund.hurdle_rate',
    'Hurdle Rate (%)',
    'product',
    'percentage',
    '{"min_value": 0.0, "max_value": 100.0}'::jsonb,
    '{"rust_type": "HurdleRate", "domain": "kyc"}'::jsonb
),
(
    'attr.hedge_fund.lock_up_period',
    'Lock-up Period (months)',
    'product',
    'integer',
    '{"required": true, "min_value": 0}'::jsonb,
    '{"rust_type": "LockUpPeriod", "domain": "kyc"}'::jsonb
);

COMMIT;

-- ============================================================================
-- VERIFICATION AND SUMMARY
-- ============================================================================

-- Display summary of inserted attributes
DO $$
DECLARE
    v_count INTEGER;
    v_category_counts TEXT;
BEGIN
    SELECT COUNT(*) INTO v_count FROM "ob-poc".attribute_registry;

    RAISE NOTICE '';
    RAISE NOTICE '==================================================================';
    RAISE NOTICE 'Attribute Registry Seeding Complete';
    RAISE NOTICE '==================================================================';
    RAISE NOTICE 'Total attributes inserted: %', v_count;
    RAISE NOTICE '';
    RAISE NOTICE 'Breakdown by category:';

    FOR v_category_counts IN
        SELECT '  - ' || category || ': ' || COUNT(*)::TEXT
        FROM "ob-poc".attribute_registry
        GROUP BY category
        ORDER BY category
    LOOP
        RAISE NOTICE '%', v_category_counts;
    END LOOP;

    RAISE NOTICE '';
    RAISE NOTICE 'All attributes are now registered and ready for use!';
    RAISE NOTICE '==================================================================';
END $$;
