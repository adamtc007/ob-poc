//! Normalization functions for Companies House data
//!
//! Converts Companies House API types to normalized structures.
//! Uses the resilient helper methods on the API types.

use super::types::{ChAddress, ChCompanyProfile, ChOfficer, ChPscRecord};
use crate::research::sources::normalized::{
    NormalizedAddress, NormalizedControlHolder, NormalizedEntity, NormalizedOfficer,
};
use chrono::NaiveDate;

/// Normalize a Companies House company profile
pub fn normalize_company(company: &ChCompanyProfile, include_raw: bool) -> NormalizedEntity {
    NormalizedEntity {
        source_key: company.company_number.clone(),
        source_name: "Companies House".to_string(),
        name: company.company_name.clone(),
        lei: None, // CH doesn't provide LEI
        registration_number: Some(company.company_number.clone()),
        tax_id: None,
        entity_type: Some(company.company_type()),
        jurisdiction: Some(map_jurisdiction(
            company.jurisdiction.as_deref().unwrap_or("england-wales"),
        )),
        status: Some(company.status()),
        incorporated_date: company.date_of_creation.as_deref().and_then(parse_ch_date),
        dissolved_date: company.date_of_cessation.as_deref().and_then(parse_ch_date),
        registered_address: company
            .registered_office_address
            .as_ref()
            .map(normalize_address),
        business_address: None, // CH doesn't distinguish business address
        raw_response: if include_raw {
            serde_json::to_value(company).ok()
        } else {
            None
        },
    }
}

/// Normalize a PSC record to a control holder
pub fn normalize_psc(psc: &ChPscRecord) -> NormalizedControlHolder {
    let (pct_low, pct_high) = psc.ownership_range();
    let (voting_low, voting_high) = psc.voting_range();

    // Use midpoint of voting range if available
    let voting_pct = match (voting_low, voting_high) {
        (Some(l), Some(h)) => Some((l + h) / rust_decimal::Decimal::from(2)),
        (Some(l), None) => Some(l),
        _ => None,
    };

    NormalizedControlHolder {
        holder_name: psc.name.clone(),
        holder_type: psc.holder_type(),
        registration_number: psc
            .identification
            .as_ref()
            .and_then(|i| i.registration_number.clone()),
        jurisdiction: psc
            .identification
            .as_ref()
            .and_then(|i| i.country_registered.clone().or(i.place_registered.clone())),
        lei: None,
        nationality: psc.nationality.clone(),
        country_of_residence: psc.country_of_residence.clone(),
        date_of_birth_partial: psc.date_of_birth.as_ref().map(|d| d.to_string_partial()),
        ownership_pct_low: pct_low,
        ownership_pct_high: pct_high,
        ownership_pct_exact: None,
        voting_pct,
        has_voting_rights: psc.has_voting_rights(),
        has_appointment_rights: psc.has_appointment_rights(),
        has_veto_rights: psc.has_significant_influence(),
        natures_of_control: psc.natures_of_control.clone(),
        notified_on: psc.notified_on.as_deref().and_then(parse_ch_date),
        ceased_on: psc.ceased_on.as_deref().and_then(parse_ch_date),
        source_document: None,
    }
}

/// Normalize an officer record
pub fn normalize_officer(officer: &ChOfficer) -> NormalizedOfficer {
    NormalizedOfficer {
        name: officer.name.clone(),
        role: officer.role(),
        appointed_date: officer.appointed_on.as_deref().and_then(parse_ch_date),
        resigned_date: officer.resigned_on.as_deref().and_then(parse_ch_date),
        nationality: officer.nationality.clone(),
        country_of_residence: officer.country_of_residence.clone(),
        date_of_birth_partial: officer
            .date_of_birth
            .as_ref()
            .map(|d| d.to_string_partial()),
        occupation: officer.occupation.clone(),
    }
}

/// Normalize a CH address
pub fn normalize_address(addr: &ChAddress) -> NormalizedAddress {
    let mut lines = Vec::new();

    if let Some(ref care_of) = addr.care_of {
        if !care_of.is_empty() {
            lines.push(format!("c/o {}", care_of));
        }
    }
    if let Some(ref premises) = addr.premises {
        if !premises.is_empty() {
            lines.push(premises.clone());
        }
    }
    if let Some(ref line1) = addr.address_line_1 {
        if !line1.is_empty() {
            lines.push(line1.clone());
        }
    }
    if let Some(ref line2) = addr.address_line_2 {
        if !line2.is_empty() {
            lines.push(line2.clone());
        }
    }

    NormalizedAddress {
        lines,
        city: addr.locality.clone(),
        region: addr.region.clone(),
        postal_code: addr.postal_code.clone(),
        country: addr.country.clone(),
    }
}

/// Map CH jurisdiction to ISO code
fn map_jurisdiction(jurisdiction: &str) -> String {
    match jurisdiction.to_lowercase().as_str() {
        "england-wales" | "england" | "wales" => "GB".to_string(),
        "scotland" => "GB".to_string(),
        "northern-ireland" => "GB".to_string(),
        "united-kingdom" => "GB".to_string(),
        other => other.to_uppercase(),
    }
}

/// Parse CH date format (YYYY-MM-DD)
fn parse_ch_date(date_str: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::sources::normalized::EntityStatus;

    #[test]
    fn test_normalize_company() {
        let company = ChCompanyProfile {
            company_number: "12345678".into(),
            company_name: "Test Company Ltd".into(),
            company_status: "active".into(),
            company_type_raw: "ltd".into(),
            jurisdiction: Some("england-wales".into()),
            date_of_creation: Some("2020-01-15".into()),
            date_of_cessation: None,
            registered_office_address: None,
            sic_codes: vec![],
            has_been_liquidated: false,
            has_charges: false,
            has_insolvency_history: false,
            registered_office_is_in_dispute: false,
            undeliverable_registered_office_address: false,
            extra: None,
        };

        let normalized = normalize_company(&company, false);

        assert_eq!(normalized.source_key, "12345678");
        assert_eq!(normalized.name, "Test Company Ltd");
        assert!(matches!(normalized.status, Some(EntityStatus::Active)));
        assert_eq!(normalized.jurisdiction, Some("GB".to_string()));
    }

    #[test]
    fn test_parse_ch_date() {
        assert_eq!(
            parse_ch_date("2020-01-15"),
            Some(NaiveDate::from_ymd_opt(2020, 1, 15).unwrap())
        );
        assert_eq!(parse_ch_date("invalid"), None);
        assert_eq!(parse_ch_date(""), None);
    }
}
