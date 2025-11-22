//! KYC (Know Your Customer) Attribute Definitions
//!
//! This module defines all typed KYC attributes using the macro system.
//! These attributes are used throughout the onboarding and compliance workflows.

use crate::{
    define_boolean_attribute, define_date_attribute, define_integer_attribute,
    define_number_attribute, define_string_attribute,
};

// ============================================================================
// IDENTITY ATTRIBUTES
// ============================================================================

define_string_attribute!(
    LegalEntityName,
    id = "attr.identity.legal_name",
    uuid = "d655aadd-3605-5490-80be-20e6202b004b",
    display_name = "Legal Entity Name",
    category = Identity,
    required = true,
    min_length = 1,
    max_length = 255,
    pattern = r"^[A-Za-z0-9\s\-\.,&'()]+$"
);

define_string_attribute!(
    FirstName,
    id = "attr.identity.first_name",
    uuid = "3020d46f-472c-5437-9647-1b0682c35935",
    display_name = "First Name",
    category = Identity,
    required = true,
    min_length = 1,
    max_length = 100,
    pattern = r"^[A-Za-z\s\-']+$"
);

define_string_attribute!(
    LastName,
    id = "attr.identity.last_name",
    uuid = "0af112fd-ec04-5938-84e8-6e5949db0b52",
    display_name = "Last Name",
    category = Identity,
    required = true,
    min_length = 1,
    max_length = 100,
    pattern = r"^[A-Za-z\s\-']+$"
);

define_date_attribute!(
    DateOfBirth,
    id = "attr.identity.date_of_birth",
    uuid = "1211e18e-fffe-5e17-9836-fb3cd70452d3",
    display_name = "Date of Birth",
    category = Identity,
    required = true
);

define_string_attribute!(
    Nationality,
    id = "attr.identity.nationality",
    uuid = "33d0752b-a92c-5e20-8559-43ab3668ecf5",
    display_name = "Nationality",
    category = Identity,
    required = true,
    min_length = 2,
    max_length = 2,
    pattern = r"^[A-Z]{2}$"
);

define_string_attribute!(
    PassportNumber,
    id = "attr.identity.passport_number",
    uuid = "c09501c7-2ea9-5ad7-b330-7d664c678e37",
    display_name = "Passport Number",
    category = Identity,
    min_length = 6,
    max_length = 20,
    pattern = r"^[A-Z0-9]+$"
);

define_string_attribute!(
    RegistrationNumber,
    id = "attr.identity.registration_number",
    uuid = "57b3ac74-182e-5ca6-b94c-46ee2a05998b",
    display_name = "Company Registration Number",
    category = Identity,
    required = true,
    min_length = 1,
    max_length = 50
);

define_date_attribute!(
    IncorporationDate,
    id = "attr.identity.incorporation_date",
    uuid = "132a5d3c-e809-5978-ab54-ccacfcbeb4aa",
    display_name = "Date of Incorporation",
    category = Identity,
    required = true
);

// ============================================================================
// ENTITY ATTRIBUTES
// ============================================================================

define_string_attribute!(
    EntityType,
    id = "attr.entity.type",
    uuid = "ce8ed47a-ef45-568f-ad5c-26483e079874",
    display_name = "Entity Type",
    category = Entity,
    required = true,
    allowed_values = ["PROPER_PERSON", "CORPORATE", "TRUST", "PARTNERSHIP", "FUND"]
);

define_string_attribute!(
    EntityDomicile,
    id = "attr.entity.domicile",
    uuid = "78883df8-c953-5d6e-90c2-c95ade0243bd",
    display_name = "Entity Domicile",
    category = Entity,
    required = true,
    min_length = 2,
    max_length = 2,
    pattern = r"^[A-Z]{2}$"
);

// ============================================================================
// KYC PROPER PERSON ATTRIBUTES
// ============================================================================

define_number_attribute!(
    ProperPersonNetWorth,
    id = "attr.kyc.proper_person.net_worth",
    uuid = "2d3a6719-3a6a-5b09-9254-7532f9f9800b",
    display_name = "Net Worth",
    category = Financial,
    required = true,
    min_value = 0.0
);

define_number_attribute!(
    ProperPersonAnnualIncome,
    id = "attr.kyc.proper_person.annual_income",
    uuid = "b1e7c730-e58b-5261-94d1-e4e2bbd81066",
    display_name = "Annual Income",
    category = Financial,
    required = true,
    min_value = 0.0
);

define_string_attribute!(
    SourceOfWealth,
    id = "attr.kyc.proper_person.source_of_wealth",
    uuid = "b78e5255-1104-5a3f-8ea0-db281b42f632",
    display_name = "Source of Wealth",
    category = Compliance,
    required = true,
    min_length = 10,
    max_length = 500
);

define_string_attribute!(
    SourceOfFunds,
    id = "attr.kyc.proper_person.source_of_funds",
    uuid = "eacb3713-acab-5aee-b9b6-ca95b35cbc1e",
    display_name = "Source of Funds",
    category = Compliance,
    required = true,
    min_length = 10,
    max_length = 500
);

define_string_attribute!(
    Occupation,
    id = "attr.kyc.proper_person.occupation",
    uuid = "9ecd482b-2298-5db1-89f0-2dd87462357c",
    display_name = "Occupation",
    category = Employment,
    required = true,
    min_length = 2,
    max_length = 100
);

// ============================================================================
// KYC CORPORATE ATTRIBUTES
// ============================================================================

define_string_attribute!(
    BusinessActivity,
    id = "attr.kyc.corporate.business_activity",
    uuid = "4f8855ff-3805-5749-98f9-874838129569",
    display_name = "Primary Business Activity",
    category = Compliance,
    required = true,
    min_length = 10,
    max_length = 500
);

define_string_attribute!(
    RegulatoryStatus,
    id = "attr.kyc.corporate.regulatory_status",
    uuid = "8fbf1610-4ca3-5e59-bc48-f2af12c8ea93",
    display_name = "Regulatory Status",
    category = Compliance,
    required = true,
    min_length = 5,
    max_length = 200
);

define_number_attribute!(
    AssetsUnderManagement,
    id = "attr.kyc.corporate.aum",
    uuid = "35ca5381-9a27-5726-9354-575a7f9a4b6e",
    display_name = "Assets Under Management",
    category = Financial,
    min_value = 0.0
);

define_integer_attribute!(
    EmployeeCount,
    id = "attr.kyc.corporate.employees_count",
    uuid = "5f372619-15e7-5f63-8774-b34e0aebb3fa",
    display_name = "Number of Employees",
    category = Employment,
    min_value = 0
);

// ============================================================================
// COMPLIANCE ATTRIBUTES
// ============================================================================

define_string_attribute!(
    FatcaStatus,
    id = "attr.compliance.fatca_status",
    uuid = "53e4858b-76d7-563d-b6d1-38d34a09aaec",
    display_name = "FATCA Status",
    category = Compliance,
    required = true,
    allowed_values = ["COMPLIANT", "NON_COMPLIANT", "EXEMPT"]
);

define_string_attribute!(
    CrsStatus,
    id = "attr.compliance.crs_status",
    uuid = "afeab5f1-f1c0-5831-85c4-390ec7c97d1d",
    display_name = "CRS Status",
    category = Compliance,
    required = true,
    allowed_values = ["COMPLIANT", "NON_COMPLIANT", "EXEMPT"]
);

define_string_attribute!(
    AmlStatus,
    id = "attr.compliance.aml_status",
    uuid = "2cd8f6fa-0c16-5271-870f-8b304bbbd8ee",
    display_name = "AML Status",
    category = Compliance,
    allowed_values = ["PASSED", "FAILED", "PENDING", "REQUIRES_REVIEW"]
);

define_string_attribute!(
    SanctionsCheck,
    id = "attr.compliance.sanctions_check",
    uuid = "80d45eb5-98d2-57a8-a693-4fbb1e3c0ea1",
    display_name = "Sanctions Screening Result",
    category = Compliance,
    allowed_values = ["CLEAR", "HIT", "PENDING"]
);

// ============================================================================
// CONTACT ATTRIBUTES
// ============================================================================

define_string_attribute!(
    Email,
    id = "attr.contact.email",
    uuid = "3e3126eb-07e4-5c0f-b8f1-c2b1552ad986",
    display_name = "Email Address",
    category = Contact,
    required = true,
    pattern = r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"
);

define_string_attribute!(
    PhoneNumber,
    id = "attr.contact.phone",
    uuid = "58144115-fd1b-5395-a530-c5a140a1281e",
    display_name = "Phone Number",
    category = Contact,
    required = true,
    pattern = r"^\+?[1-9]\d{1,14}$"
);

define_string_attribute!(
    AddressLine1,
    id = "attr.contact.address_line1",
    uuid = "7c7cdc82-b261-57c7-aee2-36e17dcd1d5d",
    display_name = "Address Line 1",
    category = Address,
    required = true,
    min_length = 5,
    max_length = 200
);

define_string_attribute!(
    AddressLine2,
    id = "attr.contact.address_line2",
    uuid = "e90b9045-d0c6-52db-989a-96ad37152e3a",
    display_name = "Address Line 2",
    category = Address,
    max_length = 200
);

define_string_attribute!(
    City,
    id = "attr.contact.city",
    uuid = "2eca2245-5d14-57b2-9a53-6f90d2b7a9d6",
    display_name = "City",
    category = Address,
    required = true,
    min_length = 2,
    max_length = 100
);

define_string_attribute!(
    PostalCode,
    id = "attr.contact.postal_code",
    uuid = "36df5ec0-f1b8-50a0-ac50-b510f0cda2fb",
    display_name = "Postal Code",
    category = Address,
    required = true,
    min_length = 3,
    max_length = 20
);

define_string_attribute!(
    Country,
    id = "attr.contact.country",
    uuid = "24e2072e-db54-547b-9e5d-0762a26261a6",
    display_name = "Country",
    category = Address,
    required = true,
    min_length = 2,
    max_length = 2,
    pattern = r"^[A-Z]{2}$"
);

// ============================================================================
// TAX ATTRIBUTES
// ============================================================================

define_string_attribute!(
    TaxIdentificationNumber,
    id = "attr.tax.tin",
    uuid = "eb142648-38b2-5c63-b1d4-1bbc251f2d50",
    display_name = "Tax Identification Number",
    category = Tax,
    required = true,
    min_length = 5,
    max_length = 50
);

define_string_attribute!(
    TaxJurisdiction,
    id = "attr.tax.jurisdiction",
    uuid = "b2e9228e-8ec7-56a0-9fbc-db5b61f0dd46",
    display_name = "Tax Jurisdiction",
    category = Tax,
    required = true,
    min_length = 2,
    max_length = 2,
    pattern = r"^[A-Z]{2}$"
);

define_boolean_attribute!(
    TaxTreatyBenefits,
    id = "attr.tax.treaty_benefits",
    uuid = "f1726fda-b3e2-579a-b5ca-c1a63f238b47",
    display_name = "Tax Treaty Benefits Eligibility",
    category = Tax
);

define_number_attribute!(
    WithholdingRate,
    id = "attr.tax.withholding_rate",
    uuid = "39d06b7b-5c7b-55e6-80b5-ca78776380d6",
    display_name = "Withholding Tax Rate",
    category = Tax,
    min_value = 0.0,
    max_value = 100.0
);

// ============================================================================
// UBO (ULTIMATE BENEFICIAL OWNER) ATTRIBUTES
// ============================================================================

define_number_attribute!(
    UboOwnershipPercentage,
    id = "attr.ubo.ownership_percentage",
    uuid = "ff6f6374-f86e-5cb5-9d0f-ad6994022ce7",
    display_name = "Ownership Percentage",
    category = UBO,
    required = true,
    min_value = 0.0,
    max_value = 100.0
);

define_string_attribute!(
    UboControlType,
    id = "attr.ubo.control_type",
    uuid = "6d6e3212-89e6-56b8-8e7c-c014b296ed70",
    display_name = "Control Type",
    category = UBO,
    required = true,
    allowed_values = ["DIRECT", "INDIRECT", "VOTING_RIGHTS", "OTHER"]
);

define_string_attribute!(
    UboFullName,
    id = "attr.ubo.full_name",
    uuid = "a554d5a5-5848-550f-a9bb-bac21b2ab944",
    display_name = "UBO Full Name",
    category = UBO,
    required = true,
    min_length = 2,
    max_length = 200
);

define_date_attribute!(
    UboDateOfBirth,
    id = "attr.ubo.date_of_birth",
    uuid = "24fcc98b-9bd1-599d-a437-1aa1f5983805",
    display_name = "UBO Date of Birth",
    category = UBO,
    required = true
);

define_string_attribute!(
    UboNationality,
    id = "attr.ubo.nationality",
    uuid = "5de8ab61-c941-5ee6-b21a-57706b3b116e",
    display_name = "UBO Nationality",
    category = UBO,
    required = true,
    min_length = 2,
    max_length = 2,
    pattern = r"^[A-Z]{2}$"
);

// ============================================================================
// RISK ATTRIBUTES
// ============================================================================

define_string_attribute!(
    RiskProfile,
    id = "attr.risk.profile",
    uuid = "03fd956a-f49c-5e3d-9be9-b1df4f1ab1b7",
    display_name = "Risk Profile",
    category = Risk,
    allowed_values = ["CONSERVATIVE", "MODERATE", "AGGRESSIVE", "SPECULATIVE"]
);

define_integer_attribute!(
    RiskTolerance,
    id = "attr.risk.tolerance",
    uuid = "ebf1665d-e64f-5d55-b44c-62a486adbfd5",
    display_name = "Risk Tolerance Score",
    category = Risk,
    min_value = 1,
    max_value = 10
);

define_integer_attribute!(
    InvestmentExperience,
    id = "attr.risk.investment_experience",
    uuid = "9847bf4f-8267-51e8-a8d9-118bc8b3f7d3",
    display_name = "Years of Investment Experience",
    category = Risk,
    required = true,
    min_value = 0
);

define_number_attribute!(
    PreviousLosses,
    id = "attr.risk.previous_losses",
    uuid = "4158c41d-5d91-5dce-baee-3ee1a5adfbaa",
    display_name = "Previous Investment Losses (%)",
    category = Risk,
    min_value = 0.0,
    max_value = 100.0
);

// ============================================================================
// BANKING ATTRIBUTES
// ============================================================================

define_string_attribute!(
    BankAccountNumber,
    id = "attr.banking.account_number",
    uuid = "d022d8f1-8ae1-55c8-84b8-e8203e17e369",
    display_name = "Bank Account Number",
    category = Financial,
    required = true,
    min_length = 8,
    max_length = 34
);

define_string_attribute!(
    Iban,
    id = "attr.banking.iban",
    uuid = "6fd0e89d-5ce9-5e96-b359-be0867643f27",
    display_name = "IBAN",
    category = Financial,
    min_length = 15,
    max_length = 34,
    pattern = r"^[A-Z]{2}[0-9]{2}[A-Z0-9]+$"
);

define_string_attribute!(
    SwiftCode,
    id = "attr.banking.swift_code",
    uuid = "7a658a7c-6865-5941-a069-277c42e10492",
    display_name = "SWIFT/BIC Code",
    category = Financial,
    required = true,
    min_length = 8,
    max_length = 11,
    pattern = r"^[A-Z]{6}[A-Z0-9]{2}([A-Z0-9]{3})?$"
);

define_string_attribute!(
    BankName,
    id = "attr.banking.bank_name",
    uuid = "5fb01b57-e622-53b4-a503-dfced456fae2",
    display_name = "Bank Name",
    category = Financial,
    required = true,
    min_length = 2,
    max_length = 200
);

// ============================================================================
// INVESTMENT ATTRIBUTES
// ============================================================================

define_number_attribute!(
    SubscriptionAmount,
    id = "attr.investment.subscription_amount",
    uuid = "e7155a74-b862-5c8b-80aa-e3d5886e375a",
    display_name = "Subscription Amount",
    category = Financial,
    required = true,
    min_value = 0.0
);

define_string_attribute!(
    SubscriptionCurrency,
    id = "attr.investment.subscription_currency",
    uuid = "9fbb97c1-68f6-5944-a4d6-73123b980362",
    display_name = "Subscription Currency",
    category = Financial,
    required = true,
    min_length = 3,
    max_length = 3,
    pattern = r"^[A-Z]{3}$"
);

define_date_attribute!(
    SubscriptionDate,
    id = "attr.investment.subscription_date",
    uuid = "23cee73e-c689-5678-9e7e-be9176957f5d",
    display_name = "Subscription Date",
    category = Financial,
    required = true
);

define_integer_attribute!(
    RedemptionNoticePeriod,
    id = "attr.investment.redemption_notice_period",
    uuid = "14b4cfcf-0e40-5b79-b4ea-81d61534f5b2",
    display_name = "Redemption Notice Period (days)",
    category = Product,
    required = true,
    min_value = 0
);

// ============================================================================
// FUND ATTRIBUTES
// ============================================================================

define_string_attribute!(
    FundName,
    id = "attr.fund.name",
    uuid = "25add16d-23be-506c-97ea-4b6696e6aba2",
    display_name = "Fund Name",
    category = Product,
    required = true,
    min_length = 2,
    max_length = 255
);

define_string_attribute!(
    FundStrategy,
    id = "attr.fund.strategy",
    uuid = "6937eb85-68bf-57e2-b4e0-1e1de38a9f1f",
    display_name = "Investment Strategy",
    category = Product,
    required = true,
    min_length = 10,
    max_length = 500
);

define_string_attribute!(
    FundBaseCurrency,
    id = "attr.fund.base_currency",
    uuid = "7e27c9e7-35ed-51ea-aabc-0bc910efe5a6",
    display_name = "Fund Base Currency",
    category = Product,
    required = true,
    min_length = 3,
    max_length = 3,
    pattern = r"^[A-Z]{3}$"
);

define_number_attribute!(
    MinimumInvestment,
    id = "attr.fund.minimum_investment",
    uuid = "464e78f4-9f7f-5e49-b031-5d95e9701317",
    display_name = "Minimum Investment Amount",
    category = Product,
    required = true,
    min_value = 0.0
);

define_number_attribute!(
    ManagementFee,
    id = "attr.fund.management_fee",
    uuid = "1e156056-7e7b-5474-8317-e1d43743c0a9",
    display_name = "Management Fee (%)",
    category = Product,
    required = true,
    min_value = 0.0,
    max_value = 100.0
);

// ============================================================================
// HEDGE FUND SPECIFIC ATTRIBUTES
// ============================================================================

define_number_attribute!(
    PerformanceFee,
    id = "attr.hedge_fund.performance_fee",
    uuid = "0d9faae2-1615-5099-bab2-891a8a9a7d9d",
    display_name = "Performance Fee (%)",
    category = Product,
    required = true,
    min_value = 0.0,
    max_value = 100.0
);

define_number_attribute!(
    HurdleRate,
    id = "attr.hedge_fund.hurdle_rate",
    uuid = "bdfddda0-2f6a-52b8-8f03-95c744d30ea3",
    display_name = "Hurdle Rate (%)",
    category = Product,
    min_value = 0.0,
    max_value = 100.0
);

define_integer_attribute!(
    LockUpPeriod,
    id = "attr.hedge_fund.lock_up_period",
    uuid = "8e8a4050-361c-5447-b28e-7816012c52c4",
    display_name = "Lock-up Period (months)",
    category = Product,
    required = true,
    min_value = 0
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::attributes::types::AttributeType;

    #[test]
    fn test_legal_entity_name() {
        assert!(LegalEntityName::validate(&"Acme Corporation Ltd".to_string()).is_ok());
        assert!(LegalEntityName::validate(&"".to_string()).is_err());
        assert!(LegalEntityName::validate(&"A".repeat(256)).is_err());
    }

    #[test]
    fn test_email_validation() {
        assert!(Email::validate(&"test@example.com".to_string()).is_ok());
        assert!(Email::validate(&"invalid-email".to_string()).is_err());
        assert!(Email::validate(&"".to_string()).is_err());
    }

    #[test]
    fn test_nationality() {
        assert!(Nationality::validate(&"GB".to_string()).is_ok());
        assert!(Nationality::validate(&"US".to_string()).is_ok());
        assert!(Nationality::validate(&"USA".to_string()).is_err()); // Too long
        assert!(Nationality::validate(&"gb".to_string()).is_err()); // Lowercase
    }

    #[test]
    fn test_ownership_percentage() {
        assert!(UboOwnershipPercentage::validate(&25.5).is_ok());
        assert!(UboOwnershipPercentage::validate(&0.0).is_ok());
        assert!(UboOwnershipPercentage::validate(&100.0).is_ok());
        assert!(UboOwnershipPercentage::validate(&-1.0).is_err());
        assert!(UboOwnershipPercentage::validate(&101.0).is_err());
    }

    #[test]
    fn test_swift_code() {
        assert!(SwiftCode::validate(&"DEUTDEFF".to_string()).is_ok());
        assert!(SwiftCode::validate(&"DEUTDEFF500".to_string()).is_ok());
        assert!(SwiftCode::validate(&"invalid".to_string()).is_err());
    }

    #[test]
    fn test_entity_type() {
        assert!(EntityType::validate(&"CORPORATE".to_string()).is_ok());
        assert!(EntityType::validate(&"TRUST".to_string()).is_ok());
        assert!(EntityType::validate(&"INVALID".to_string()).is_err());
    }

    #[test]
    fn test_attribute_metadata() {
        let metadata = FirstName::metadata();
        assert_eq!(metadata.id, "attr.identity.first_name");
        assert_eq!(metadata.display_name, "First Name");
    }
}
