//! SEC EDGAR API response types
//!
//! # Resilience Pattern
//!
//! This module follows the "store raw, map lazily" pattern:
//! - All enum-like fields stored as String
//! - Liberal use of `#[serde(default)]`
//! - Helper methods for typed access

use serde::{Deserialize, Serialize};

// =============================================================================
// Company Submissions
// =============================================================================

/// Company submissions from GET /submissions/CIK{cik}.json
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecCompanySubmissions {
    pub cik: String,
    pub name: String,
    #[serde(rename = "entityType", default)]
    pub entity_type: Option<String>,
    #[serde(default)]
    pub sic: Option<String>,
    #[serde(rename = "sicDescription", default)]
    pub sic_description: Option<String>,
    #[serde(default)]
    pub tickers: Vec<String>,
    #[serde(default)]
    pub exchanges: Vec<String>,
    #[serde(default)]
    pub ein: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(rename = "fiscalYearEnd", default)]
    pub fiscal_year_end: Option<String>,
    #[serde(rename = "stateOfIncorporation", default)]
    pub state_of_incorporation: Option<String>,
    #[serde(rename = "stateOfIncorporationDescription", default)]
    pub state_of_incorporation_description: Option<String>,
    #[serde(default)]
    pub phone: Option<String>,
    pub filings: SecFilings,
    #[serde(default)]
    pub addresses: Option<SecAddresses>,
}

impl SecCompanySubmissions {
    /// Get the CIK padded to 10 digits
    pub fn cik_padded(&self) -> String {
        format!("{:0>10}", self.cik.trim_start_matches('0'))
    }

    /// Check if this is an operating company (vs investment company, etc.)
    pub fn is_operating_company(&self) -> bool {
        self.entity_type
            .as_ref()
            .is_none_or(|t| t.to_lowercase() == "operating")
    }
}

// =============================================================================
// Filings
// =============================================================================

/// Filing information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecFilings {
    pub recent: SecRecentFilings,
    #[serde(default)]
    pub files: Vec<SecFilingFile>,
}

/// Recent filings arrays - parallel arrays for efficiency
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecRecentFilings {
    #[serde(rename = "accessionNumber", default)]
    pub accession_number: Vec<String>,
    #[serde(default)]
    pub form: Vec<String>,
    #[serde(rename = "filingDate", default)]
    pub filing_date: Vec<String>,
    #[serde(rename = "reportDate", default)]
    pub report_date: Vec<String>,
    #[serde(rename = "acceptanceDateTime", default)]
    pub acceptance_date_time: Vec<String>,
    #[serde(rename = "primaryDocument", default)]
    pub primary_document: Vec<String>,
    #[serde(rename = "primaryDocDescription", default)]
    pub primary_doc_description: Vec<String>,
}

impl SecRecentFilings {
    /// Get filing at index as structured object
    pub fn get(&self, index: usize) -> Option<SecFilingInfo> {
        if index >= self.accession_number.len() {
            return None;
        }
        Some(SecFilingInfo {
            accession_number: self.accession_number.get(index)?.clone(),
            form: self.form.get(index).cloned().unwrap_or_default(),
            filing_date: self.filing_date.get(index).cloned().unwrap_or_default(),
            primary_document: self
                .primary_document
                .get(index)
                .cloned()
                .unwrap_or_default(),
        })
    }

    /// Iterate over all filings
    pub fn iter(&self) -> impl Iterator<Item = SecFilingInfo> + '_ {
        (0..self.accession_number.len()).filter_map(|i| self.get(i))
    }

    /// Filter to 13D/13G filings
    pub fn beneficial_ownership_filings(&self) -> Vec<SecFilingInfo> {
        self.iter()
            .filter(|f| f.is_beneficial_ownership_filing())
            .collect()
    }
}

/// Structured filing info extracted from parallel arrays
#[derive(Debug, Clone)]
pub struct SecFilingInfo {
    pub accession_number: String,
    pub form: String,
    pub filing_date: String,
    pub primary_document: String,
}

impl SecFilingInfo {
    /// Check if this is a 13D/13G beneficial ownership filing
    pub fn is_beneficial_ownership_filing(&self) -> bool {
        let form_upper = self.form.to_uppercase();
        form_upper.starts_with("SC 13D")
            || form_upper.starts_with("SC 13G")
            || form_upper.starts_with("SC13D")
            || form_upper.starts_with("SC13G")
    }

    /// Check if this is an amendment
    pub fn is_amendment(&self) -> bool {
        self.form.contains("/A")
    }
}

/// Additional filing files (for paginated results)
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecFilingFile {
    pub name: String,
    #[serde(rename = "filingCount", default)]
    pub filing_count: i32,
    #[serde(rename = "filingFrom", default)]
    pub filing_from: String,
    #[serde(rename = "filingTo", default)]
    pub filing_to: String,
}

// =============================================================================
// Addresses
// =============================================================================

/// Company addresses
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SecAddresses {
    #[serde(default)]
    pub mailing: Option<SecAddress>,
    #[serde(default)]
    pub business: Option<SecAddress>,
}

/// Individual address
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SecAddress {
    #[serde(default)]
    pub street1: Option<String>,
    #[serde(default)]
    pub street2: Option<String>,
    #[serde(default)]
    pub city: Option<String>,
    #[serde(rename = "stateOrCountry", default)]
    pub state_or_country: Option<String>,
    #[serde(rename = "zipCode", default)]
    pub zip_code: Option<String>,
    #[serde(rename = "stateOrCountryDescription", default)]
    pub state_or_country_description: Option<String>,
}

// =============================================================================
// Beneficial Ownership (13D/13G)
// =============================================================================

/// Parsed beneficial owner from 13D/13G filing
/// Note: These are extracted from semi-structured XML/SGML documents
#[allow(dead_code)] // Will be used when 13D/13G parsing is implemented
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecBeneficialOwner {
    /// Name of the filer/beneficial owner
    pub filer_name: String,
    /// Type: IN (individual), CO (corporation), etc.
    pub filer_type: Option<String>,
    /// Filer address
    pub filer_address: Option<String>,
    /// Issuer (subject company) name
    pub issuer_name: String,
    /// CUSIP of the security
    pub issuer_cusip: Option<String>,
    /// Percentage of class owned
    pub percent_of_class: Option<rust_decimal::Decimal>,
    /// Number of shares beneficially owned
    pub shares_beneficially_owned: Option<i64>,
    /// Sole voting power
    pub sole_voting_power: Option<i64>,
    /// Shared voting power
    pub shared_voting_power: Option<i64>,
    /// Sole dispositive power
    pub sole_dispositive_power: Option<i64>,
    /// Shared dispositive power
    pub shared_dispositive_power: Option<i64>,
    /// Filing date
    pub filing_date: String,
    /// Form type (SC 13D, SC 13G, SC 13D/A, etc.)
    pub form_type: String,
    /// Accession number
    pub accession_number: String,
}

#[allow(dead_code)] // Will be used when 13D/13G parsing is implemented
impl SecBeneficialOwner {
    /// Check if this is an individual filer
    pub fn is_individual(&self) -> bool {
        self.filer_type
            .as_ref()
            .is_some_and(|t| t.to_uppercase() == "IN")
    }

    /// Check if this is a corporate filer
    pub fn is_corporate(&self) -> bool {
        self.filer_type
            .as_ref()
            .is_some_and(|t| matches!(t.to_uppercase().as_str(), "CO" | "HC" | "IA" | "BD"))
    }

    /// Get total voting power
    pub fn total_voting_power(&self) -> Option<i64> {
        match (self.sole_voting_power, self.shared_voting_power) {
            (Some(sole), Some(shared)) => Some(sole + shared),
            (Some(sole), None) => Some(sole),
            (None, Some(shared)) => Some(shared),
            (None, None) => None,
        }
    }
}

// =============================================================================
// Company Ticker Search
// =============================================================================

/// Company ticker lookup result
#[allow(dead_code)] // Will be used for ticker-based company lookup
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecTickerLookup {
    #[serde(default)]
    pub cik_str: String,
    #[serde(default)]
    pub ticker: String,
    #[serde(default)]
    pub title: String,
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cik_padded() {
        let submissions = SecCompanySubmissions {
            cik: "320193".into(),
            name: "Apple Inc.".into(),
            entity_type: None,
            sic: None,
            sic_description: None,
            tickers: vec![],
            exchanges: vec![],
            ein: None,
            category: None,
            fiscal_year_end: None,
            state_of_incorporation: None,
            state_of_incorporation_description: None,
            phone: None,
            filings: SecFilings {
                recent: SecRecentFilings {
                    accession_number: vec![],
                    form: vec![],
                    filing_date: vec![],
                    report_date: vec![],
                    acceptance_date_time: vec![],
                    primary_document: vec![],
                    primary_doc_description: vec![],
                },
                files: vec![],
            },
            addresses: None,
        };

        assert_eq!(submissions.cik_padded(), "0000320193");
    }

    #[test]
    fn test_filing_info_is_beneficial_ownership() {
        let filing = SecFilingInfo {
            accession_number: "0001234567-24-000001".into(),
            form: "SC 13D".into(),
            filing_date: "2024-01-15".into(),
            primary_document: "sc13d.htm".into(),
        };
        assert!(filing.is_beneficial_ownership_filing());

        let filing = SecFilingInfo {
            accession_number: "0001234567-24-000002".into(),
            form: "10-K".into(),
            filing_date: "2024-01-15".into(),
            primary_document: "10k.htm".into(),
        };
        assert!(!filing.is_beneficial_ownership_filing());
    }

    #[test]
    fn test_deserialize_submissions() {
        let json = r#"{
            "cik": "0000320193",
            "name": "Apple Inc.",
            "entityType": "operating",
            "sic": "3571",
            "sicDescription": "ELECTRONIC COMPUTERS",
            "tickers": ["AAPL"],
            "exchanges": ["Nasdaq"],
            "filings": {
                "recent": {
                    "accessionNumber": ["0000320193-24-000001"],
                    "form": ["10-K"],
                    "filingDate": ["2024-01-15"],
                    "primaryDocument": ["aapl-20240115.htm"]
                }
            }
        }"#;

        let submissions: SecCompanySubmissions = serde_json::from_str(json).unwrap();
        assert_eq!(submissions.cik, "0000320193");
        assert_eq!(submissions.name, "Apple Inc.");
        assert_eq!(submissions.tickers, vec!["AAPL".to_string()]);
    }

    #[test]
    fn test_deserialize_minimal() {
        // Should handle minimal response
        let json = r#"{
            "cik": "320193",
            "name": "Apple Inc.",
            "filings": {
                "recent": {}
            }
        }"#;

        let submissions: SecCompanySubmissions = serde_json::from_str(json).unwrap();
        assert_eq!(submissions.cik, "320193");
        assert!(submissions.tickers.is_empty());
    }
}
