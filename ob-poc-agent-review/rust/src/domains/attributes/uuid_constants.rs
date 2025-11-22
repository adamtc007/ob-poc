//! Auto-generated UUID constants for attributes
//! Phase 0 & 2: Complete mapping of all 59 attributes
//! Generated: 2025-11-14

use std::collections::HashMap;
use uuid::Uuid;

pub const IDENTITY_LEGAL_NAME_UUID: &str = "d655aadd-3605-5490-80be-20e6202b004b";
pub const IDENTITY_FIRST_NAME_UUID: &str = "3020d46f-472c-5437-9647-1b0682c35935";
pub const IDENTITY_LAST_NAME_UUID: &str = "0af112fd-ec04-5938-84e8-6e5949db0b52";
pub const IDENTITY_DATE_OF_BIRTH_UUID: &str = "1211e18e-fffe-5e17-9836-fb3cd70452d3";
pub const IDENTITY_NATIONALITY_UUID: &str = "33d0752b-a92c-5e20-8559-43ab3668ecf5";
pub const IDENTITY_PASSPORT_NUMBER_UUID: &str = "c09501c7-2ea9-5ad7-b330-7d664c678e37";
pub const IDENTITY_REGISTRATION_NUMBER_UUID: &str = "57b3ac74-182e-5ca6-b94c-46ee2a05998b";
pub const IDENTITY_INCORPORATION_DATE_UUID: &str = "132a5d3c-e809-5978-ab54-ccacfcbeb4aa";
pub const ENTITY_TYPE_UUID: &str = "ce8ed47a-ef45-568f-ad5c-26483e079874";
pub const ENTITY_DOMICILE_UUID: &str = "78883df8-c953-5d6e-90c2-c95ade0243bd";
pub const KYC_PROPER_PERSON_NET_WORTH_UUID: &str = "2d3a6719-3a6a-5b09-9254-7532f9f9800b";
pub const KYC_PROPER_PERSON_ANNUAL_INCOME_UUID: &str = "b1e7c730-e58b-5261-94d1-e4e2bbd81066";
pub const KYC_PROPER_PERSON_SOURCE_OF_WEALTH_UUID: &str = "b78e5255-1104-5a3f-8ea0-db281b42f632";
pub const KYC_PROPER_PERSON_SOURCE_OF_FUNDS_UUID: &str = "eacb3713-acab-5aee-b9b6-ca95b35cbc1e";
pub const KYC_PROPER_PERSON_OCCUPATION_UUID: &str = "9ecd482b-2298-5db1-89f0-2dd87462357c";
pub const KYC_CORPORATE_BUSINESS_ACTIVITY_UUID: &str = "4f8855ff-3805-5749-98f9-874838129569";
pub const KYC_CORPORATE_REGULATORY_STATUS_UUID: &str = "8fbf1610-4ca3-5e59-bc48-f2af12c8ea93";
pub const KYC_CORPORATE_AUM_UUID: &str = "35ca5381-9a27-5726-9354-575a7f9a4b6e";
pub const KYC_CORPORATE_EMPLOYEES_COUNT_UUID: &str = "5f372619-15e7-5f63-8774-b34e0aebb3fa";
pub const COMPLIANCE_FATCA_STATUS_UUID: &str = "53e4858b-76d7-563d-b6d1-38d34a09aaec";
pub const COMPLIANCE_CRS_STATUS_UUID: &str = "afeab5f1-f1c0-5831-85c4-390ec7c97d1d";
pub const COMPLIANCE_AML_STATUS_UUID: &str = "2cd8f6fa-0c16-5271-870f-8b304bbbd8ee";
pub const COMPLIANCE_SANCTIONS_CHECK_UUID: &str = "80d45eb5-98d2-57a8-a693-4fbb1e3c0ea1";
pub const CONTACT_EMAIL_UUID: &str = "3e3126eb-07e4-5c0f-b8f1-c2b1552ad986";
pub const CONTACT_PHONE_UUID: &str = "58144115-fd1b-5395-a530-c5a140a1281e";
pub const CONTACT_ADDRESS_LINE1_UUID: &str = "7c7cdc82-b261-57c7-aee2-36e17dcd1d5d";
pub const CONTACT_ADDRESS_LINE2_UUID: &str = "e90b9045-d0c6-52db-989a-96ad37152e3a";
pub const CONTACT_CITY_UUID: &str = "2eca2245-5d14-57b2-9a53-6f90d2b7a9d6";
pub const CONTACT_POSTAL_CODE_UUID: &str = "36df5ec0-f1b8-50a0-ac50-b510f0cda2fb";
pub const CONTACT_COUNTRY_UUID: &str = "24e2072e-db54-547b-9e5d-0762a26261a6";
pub const TAX_TIN_UUID: &str = "eb142648-38b2-5c63-b1d4-1bbc251f2d50";
pub const TAX_JURISDICTION_UUID: &str = "b2e9228e-8ec7-56a0-9fbc-db5b61f0dd46";
pub const TAX_TREATY_BENEFITS_UUID: &str = "f1726fda-b3e2-579a-b5ca-c1a63f238b47";
pub const TAX_WITHHOLDING_RATE_UUID: &str = "39d06b7b-5c7b-55e6-80b5-ca78776380d6";
pub const UBO_OWNERSHIP_PERCENTAGE_UUID: &str = "ff6f6374-f86e-5cb5-9d0f-ad6994022ce7";
pub const UBO_CONTROL_TYPE_UUID: &str = "6d6e3212-89e6-56b8-8e7c-c014b296ed70";
pub const UBO_FULL_NAME_UUID: &str = "a554d5a5-5848-550f-a9bb-bac21b2ab944";
pub const UBO_DATE_OF_BIRTH_UUID: &str = "24fcc98b-9bd1-599d-a437-1aa1f5983805";
pub const UBO_NATIONALITY_UUID: &str = "5de8ab61-c941-5ee6-b21a-57706b3b116e";
pub const RISK_PROFILE_UUID: &str = "03fd956a-f49c-5e3d-9be9-b1df4f1ab1b7";
pub const RISK_TOLERANCE_UUID: &str = "ebf1665d-e64f-5d55-b44c-62a486adbfd5";
pub const RISK_INVESTMENT_EXPERIENCE_UUID: &str = "9847bf4f-8267-51e8-a8d9-118bc8b3f7d3";
pub const RISK_PREVIOUS_LOSSES_UUID: &str = "4158c41d-5d91-5dce-baee-3ee1a5adfbaa";
pub const BANKING_ACCOUNT_NUMBER_UUID: &str = "d022d8f1-8ae1-55c8-84b8-e8203e17e369";
pub const BANKING_IBAN_UUID: &str = "6fd0e89d-5ce9-5e96-b359-be0867643f27";
pub const BANKING_SWIFT_CODE_UUID: &str = "7a658a7c-6865-5941-a069-277c42e10492";
pub const BANKING_BANK_NAME_UUID: &str = "5fb01b57-e622-53b4-a503-dfced456fae2";
pub const INVESTMENT_SUBSCRIPTION_AMOUNT_UUID: &str = "e7155a74-b862-5c8b-80aa-e3d5886e375a";
pub const INVESTMENT_SUBSCRIPTION_CURRENCY_UUID: &str = "9fbb97c1-68f6-5944-a4d6-73123b980362";
pub const INVESTMENT_SUBSCRIPTION_DATE_UUID: &str = "23cee73e-c689-5678-9e7e-be9176957f5d";
pub const INVESTMENT_REDEMPTION_NOTICE_PERIOD_UUID: &str = "14b4cfcf-0e40-5b79-b4ea-81d61534f5b2";
pub const FUND_NAME_UUID: &str = "25add16d-23be-506c-97ea-4b6696e6aba2";
pub const FUND_STRATEGY_UUID: &str = "6937eb85-68bf-57e2-b4e0-1e1de38a9f1f";
pub const FUND_BASE_CURRENCY_UUID: &str = "7e27c9e7-35ed-51ea-aabc-0bc910efe5a6";
pub const FUND_MINIMUM_INVESTMENT_UUID: &str = "464e78f4-9f7f-5e49-b031-5d95e9701317";
pub const FUND_MANAGEMENT_FEE_UUID: &str = "1e156056-7e7b-5474-8317-e1d43743c0a9";
pub const HEDGE_FUND_PERFORMANCE_FEE_UUID: &str = "0d9faae2-1615-5099-bab2-891a8a9a7d9d";
pub const HEDGE_FUND_HURDLE_RATE_UUID: &str = "bdfddda0-2f6a-52b8-8f03-95c744d30ea3";
pub const HEDGE_FUND_LOCK_UP_PERIOD_UUID: &str = "8e8a4050-361c-5447-b28e-7816012c52c4";

/// Build complete UUID mapping for all 59 attributes
pub fn build_uuid_map() -> HashMap<String, Uuid> {
    let mut map = HashMap::new();

    map.insert("attr.identity.legal_name".to_string(), Uuid::parse_str(IDENTITY_LEGAL_NAME_UUID).unwrap());
    map.insert("attr.identity.first_name".to_string(), Uuid::parse_str(IDENTITY_FIRST_NAME_UUID).unwrap());
    map.insert("attr.identity.last_name".to_string(), Uuid::parse_str(IDENTITY_LAST_NAME_UUID).unwrap());
    map.insert("attr.identity.date_of_birth".to_string(), Uuid::parse_str(IDENTITY_DATE_OF_BIRTH_UUID).unwrap());
    map.insert("attr.identity.nationality".to_string(), Uuid::parse_str(IDENTITY_NATIONALITY_UUID).unwrap());
    map.insert("attr.identity.passport_number".to_string(), Uuid::parse_str(IDENTITY_PASSPORT_NUMBER_UUID).unwrap());
    map.insert("attr.identity.registration_number".to_string(), Uuid::parse_str(IDENTITY_REGISTRATION_NUMBER_UUID).unwrap());
    map.insert("attr.identity.incorporation_date".to_string(), Uuid::parse_str(IDENTITY_INCORPORATION_DATE_UUID).unwrap());
    map.insert("attr.entity.type".to_string(), Uuid::parse_str(ENTITY_TYPE_UUID).unwrap());
    map.insert("attr.entity.domicile".to_string(), Uuid::parse_str(ENTITY_DOMICILE_UUID).unwrap());
    map.insert("attr.kyc.proper_person.net_worth".to_string(), Uuid::parse_str(KYC_PROPER_PERSON_NET_WORTH_UUID).unwrap());
    map.insert("attr.kyc.proper_person.annual_income".to_string(), Uuid::parse_str(KYC_PROPER_PERSON_ANNUAL_INCOME_UUID).unwrap());
    map.insert("attr.kyc.proper_person.source_of_wealth".to_string(), Uuid::parse_str(KYC_PROPER_PERSON_SOURCE_OF_WEALTH_UUID).unwrap());
    map.insert("attr.kyc.proper_person.source_of_funds".to_string(), Uuid::parse_str(KYC_PROPER_PERSON_SOURCE_OF_FUNDS_UUID).unwrap());
    map.insert("attr.kyc.proper_person.occupation".to_string(), Uuid::parse_str(KYC_PROPER_PERSON_OCCUPATION_UUID).unwrap());
    map.insert("attr.kyc.corporate.business_activity".to_string(), Uuid::parse_str(KYC_CORPORATE_BUSINESS_ACTIVITY_UUID).unwrap());
    map.insert("attr.kyc.corporate.regulatory_status".to_string(), Uuid::parse_str(KYC_CORPORATE_REGULATORY_STATUS_UUID).unwrap());
    map.insert("attr.kyc.corporate.aum".to_string(), Uuid::parse_str(KYC_CORPORATE_AUM_UUID).unwrap());
    map.insert("attr.kyc.corporate.employees_count".to_string(), Uuid::parse_str(KYC_CORPORATE_EMPLOYEES_COUNT_UUID).unwrap());
    map.insert("attr.compliance.fatca_status".to_string(), Uuid::parse_str(COMPLIANCE_FATCA_STATUS_UUID).unwrap());
    map.insert("attr.compliance.crs_status".to_string(), Uuid::parse_str(COMPLIANCE_CRS_STATUS_UUID).unwrap());
    map.insert("attr.compliance.aml_status".to_string(), Uuid::parse_str(COMPLIANCE_AML_STATUS_UUID).unwrap());
    map.insert("attr.compliance.sanctions_check".to_string(), Uuid::parse_str(COMPLIANCE_SANCTIONS_CHECK_UUID).unwrap());
    map.insert("attr.contact.email".to_string(), Uuid::parse_str(CONTACT_EMAIL_UUID).unwrap());
    map.insert("attr.contact.phone".to_string(), Uuid::parse_str(CONTACT_PHONE_UUID).unwrap());
    map.insert("attr.contact.address_line1".to_string(), Uuid::parse_str(CONTACT_ADDRESS_LINE1_UUID).unwrap());
    map.insert("attr.contact.address_line2".to_string(), Uuid::parse_str(CONTACT_ADDRESS_LINE2_UUID).unwrap());
    map.insert("attr.contact.city".to_string(), Uuid::parse_str(CONTACT_CITY_UUID).unwrap());
    map.insert("attr.contact.postal_code".to_string(), Uuid::parse_str(CONTACT_POSTAL_CODE_UUID).unwrap());
    map.insert("attr.contact.country".to_string(), Uuid::parse_str(CONTACT_COUNTRY_UUID).unwrap());
    map.insert("attr.tax.tin".to_string(), Uuid::parse_str(TAX_TIN_UUID).unwrap());
    map.insert("attr.tax.jurisdiction".to_string(), Uuid::parse_str(TAX_JURISDICTION_UUID).unwrap());
    map.insert("attr.tax.treaty_benefits".to_string(), Uuid::parse_str(TAX_TREATY_BENEFITS_UUID).unwrap());
    map.insert("attr.tax.withholding_rate".to_string(), Uuid::parse_str(TAX_WITHHOLDING_RATE_UUID).unwrap());
    map.insert("attr.ubo.ownership_percentage".to_string(), Uuid::parse_str(UBO_OWNERSHIP_PERCENTAGE_UUID).unwrap());
    map.insert("attr.ubo.control_type".to_string(), Uuid::parse_str(UBO_CONTROL_TYPE_UUID).unwrap());
    map.insert("attr.ubo.full_name".to_string(), Uuid::parse_str(UBO_FULL_NAME_UUID).unwrap());
    map.insert("attr.ubo.date_of_birth".to_string(), Uuid::parse_str(UBO_DATE_OF_BIRTH_UUID).unwrap());
    map.insert("attr.ubo.nationality".to_string(), Uuid::parse_str(UBO_NATIONALITY_UUID).unwrap());
    map.insert("attr.risk.profile".to_string(), Uuid::parse_str(RISK_PROFILE_UUID).unwrap());
    map.insert("attr.risk.tolerance".to_string(), Uuid::parse_str(RISK_TOLERANCE_UUID).unwrap());
    map.insert("attr.risk.investment_experience".to_string(), Uuid::parse_str(RISK_INVESTMENT_EXPERIENCE_UUID).unwrap());
    map.insert("attr.risk.previous_losses".to_string(), Uuid::parse_str(RISK_PREVIOUS_LOSSES_UUID).unwrap());
    map.insert("attr.banking.account_number".to_string(), Uuid::parse_str(BANKING_ACCOUNT_NUMBER_UUID).unwrap());
    map.insert("attr.banking.iban".to_string(), Uuid::parse_str(BANKING_IBAN_UUID).unwrap());
    map.insert("attr.banking.swift_code".to_string(), Uuid::parse_str(BANKING_SWIFT_CODE_UUID).unwrap());
    map.insert("attr.banking.bank_name".to_string(), Uuid::parse_str(BANKING_BANK_NAME_UUID).unwrap());
    map.insert("attr.investment.subscription_amount".to_string(), Uuid::parse_str(INVESTMENT_SUBSCRIPTION_AMOUNT_UUID).unwrap());
    map.insert("attr.investment.subscription_currency".to_string(), Uuid::parse_str(INVESTMENT_SUBSCRIPTION_CURRENCY_UUID).unwrap());
    map.insert("attr.investment.subscription_date".to_string(), Uuid::parse_str(INVESTMENT_SUBSCRIPTION_DATE_UUID).unwrap());
    map.insert("attr.investment.redemption_notice_period".to_string(), Uuid::parse_str(INVESTMENT_REDEMPTION_NOTICE_PERIOD_UUID).unwrap());
    map.insert("attr.fund.name".to_string(), Uuid::parse_str(FUND_NAME_UUID).unwrap());
    map.insert("attr.fund.strategy".to_string(), Uuid::parse_str(FUND_STRATEGY_UUID).unwrap());
    map.insert("attr.fund.base_currency".to_string(), Uuid::parse_str(FUND_BASE_CURRENCY_UUID).unwrap());
    map.insert("attr.fund.minimum_investment".to_string(), Uuid::parse_str(FUND_MINIMUM_INVESTMENT_UUID).unwrap());
    map.insert("attr.fund.management_fee".to_string(), Uuid::parse_str(FUND_MANAGEMENT_FEE_UUID).unwrap());
    map.insert("attr.hedge_fund.performance_fee".to_string(), Uuid::parse_str(HEDGE_FUND_PERFORMANCE_FEE_UUID).unwrap());
    map.insert("attr.hedge_fund.hurdle_rate".to_string(), Uuid::parse_str(HEDGE_FUND_HURDLE_RATE_UUID).unwrap());
    map.insert("attr.hedge_fund.lock_up_period".to_string(), Uuid::parse_str(HEDGE_FUND_LOCK_UP_PERIOD_UUID).unwrap());

    map
}

/// Build reverse mapping: UUID to semantic ID
pub fn build_semantic_map() -> HashMap<Uuid, String> {
    build_uuid_map().into_iter().map(|(k, v)| (v, k)).collect()
}

/// Helper functions for resolution
pub fn semantic_to_uuid(semantic_id: &str) -> Option<Uuid> {
    build_uuid_map().get(semantic_id).copied()
}

pub fn uuid_to_semantic(uuid: &Uuid) -> Option<String> {
    build_semantic_map().get(uuid).cloned()
}
