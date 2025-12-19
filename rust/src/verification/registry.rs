//! Registry Verification Stubs
//!
//! Provides interfaces for verifying entity information against external
//! authoritative registries like GLEIF (LEI), company houses, and
//! government databases.
//!
//! ## Supported Registries
//!
//! - **GLEIF**: Legal Entity Identifier registry
//! - **Company House**: UK company registry
//! - **SEC EDGAR**: US public company filings
//! - **OpenCorporates**: Aggregated company data
//!
//! Note: These are stub implementations. Actual API integrations would
//! require credentials and network access.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Registry Types
// ============================================================================

/// Supported registry types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RegistryType {
    /// GLEIF - Legal Entity Identifier
    Gleif,
    /// UK Companies House
    CompaniesHouseUk,
    /// US SEC EDGAR
    SecEdgar,
    /// OpenCorporates aggregated data
    OpenCorporates,
    /// Luxembourg Business Register
    LuxembourgRcs,
    /// Irish Companies Registration Office
    IrishCro,
    /// Generic government registry
    GovernmentRegistry,
}

impl std::fmt::Display for RegistryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryType::Gleif => write!(f, "GLEIF"),
            RegistryType::CompaniesHouseUk => write!(f, "COMPANIES_HOUSE_UK"),
            RegistryType::SecEdgar => write!(f, "SEC_EDGAR"),
            RegistryType::OpenCorporates => write!(f, "OPEN_CORPORATES"),
            RegistryType::LuxembourgRcs => write!(f, "LUXEMBOURG_RCS"),
            RegistryType::IrishCro => write!(f, "IRISH_CRO"),
            RegistryType::GovernmentRegistry => write!(f, "GOVERNMENT_REGISTRY"),
        }
    }
}

// ============================================================================
// Registry Check Result
// ============================================================================

/// Result of a registry verification check
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryCheckResult {
    /// Entity that was checked
    pub entity_id: Uuid,

    /// Registry used for verification
    pub registry: RegistryType,

    /// Whether entity was found in registry
    pub found: bool,

    /// Whether claimed data matches registry
    pub matches: Option<bool>,

    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,

    /// Specific field comparison results
    pub field_results: Vec<FieldCheckResult>,

    /// Raw registry data (if available)
    pub registry_data: Option<serde_json::Value>,

    /// When the check was performed
    pub checked_at: DateTime<Utc>,

    /// Error message if check failed
    pub error: Option<String>,
}

/// Result of checking a specific field
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldCheckResult {
    /// Field name (e.g., "legal_name", "jurisdiction", "registration_number")
    pub field: String,

    /// Value claimed by client
    pub claimed_value: Option<String>,

    /// Value from registry
    pub registry_value: Option<String>,

    /// Whether they match
    pub matches: bool,

    /// Match type
    pub match_type: FieldMatchType,
}

/// Type of field match
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FieldMatchType {
    /// Values are exactly the same
    ExactMatch,
    /// Values are similar (fuzzy match)
    FuzzyMatch,
    /// Minor variation (spelling, formatting)
    MinorVariation,
    /// Values are different
    Mismatch,
    /// Field not available in registry
    NotAvailable,
    /// Field not provided by client
    NotProvided,
}

// ============================================================================
// GLEIF Data Structures
// ============================================================================

/// GLEIF entity data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GleifEntity {
    /// Legal Entity Identifier (20 characters)
    pub lei: String,

    /// Legal name
    pub legal_name: String,

    /// Jurisdiction of incorporation
    pub jurisdiction: String,

    /// Entity category (FUND, GENERAL, etc.)
    pub category: Option<String>,

    /// Entity status (ACTIVE, INACTIVE)
    pub status: String,

    /// Registration number
    pub registration_number: Option<String>,

    /// Legal address
    pub legal_address: Option<GleifAddress>,

    /// Headquarters address
    pub headquarters_address: Option<GleifAddress>,

    /// Registration details
    pub registration: GleifRegistration,
}

/// GLEIF address structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GleifAddress {
    pub city: Option<String>,
    pub country: Option<String>,
    pub postal_code: Option<String>,
    pub address_lines: Vec<String>,
}

/// GLEIF registration details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GleifRegistration {
    /// Registration status
    pub status: String,

    /// Corroboration level
    pub corroboration_level: String,

    /// Initial registration date
    pub initial_registration_date: Option<DateTime<Utc>>,

    /// Last update date
    pub last_update_date: Option<DateTime<Utc>>,

    /// Next renewal date
    pub next_renewal_date: Option<DateTime<Utc>>,

    /// Managing LOU (Local Operating Unit)
    pub managing_lou: Option<String>,
}

// ============================================================================
// Registry Verifier
// ============================================================================

/// Verifies entity information against external registries
pub struct RegistryVerifier {
    /// GLEIF API base URL
    gleif_base_url: String,
}

impl RegistryVerifier {
    /// Create a new verifier with default configuration
    pub fn new() -> Self {
        Self {
            gleif_base_url: "https://api.gleif.org/api/v1".to_string(),
        }
    }

    /// Create a verifier with custom GLEIF URL
    pub fn with_gleif_url(gleif_url: &str) -> Self {
        Self {
            gleif_base_url: gleif_url.to_string(),
        }
    }

    /// Verify an entity against GLEIF by LEI
    ///
    /// This is a stub implementation. In production, this would make
    /// an HTTP request to the GLEIF API.
    pub async fn verify_gleif_by_lei(
        &self,
        entity_id: Uuid,
        lei: &str,
        claimed_name: Option<&str>,
        claimed_jurisdiction: Option<&str>,
    ) -> RegistryCheckResult {
        // Validate LEI format (20 alphanumeric characters)
        if lei.len() != 20 || !lei.chars().all(|c| c.is_alphanumeric()) {
            return RegistryCheckResult {
                entity_id,
                registry: RegistryType::Gleif,
                found: false,
                matches: None,
                confidence: 0.0,
                field_results: vec![],
                registry_data: None,
                checked_at: Utc::now(),
                error: Some(format!("Invalid LEI format: {}", lei)),
            };
        }

        // STUB: In production, this would call:
        // GET https://api.gleif.org/api/v1/lei-records/{lei}
        //
        // For now, return a simulated "not found" result
        // to indicate the verification was attempted but
        // the actual API call is not implemented.

        RegistryCheckResult {
            entity_id,
            registry: RegistryType::Gleif,
            found: false,
            matches: None,
            confidence: 0.0,
            field_results: vec![
                FieldCheckResult {
                    field: "lei".to_string(),
                    claimed_value: Some(lei.to_string()),
                    registry_value: None,
                    matches: false,
                    match_type: FieldMatchType::NotAvailable,
                },
                FieldCheckResult {
                    field: "legal_name".to_string(),
                    claimed_value: claimed_name.map(|s| s.to_string()),
                    registry_value: None,
                    matches: false,
                    match_type: FieldMatchType::NotAvailable,
                },
                FieldCheckResult {
                    field: "jurisdiction".to_string(),
                    claimed_value: claimed_jurisdiction.map(|s| s.to_string()),
                    registry_value: None,
                    matches: false,
                    match_type: FieldMatchType::NotAvailable,
                },
            ],
            registry_data: None,
            checked_at: Utc::now(),
            error: Some("GLEIF API integration not implemented - stub only".to_string()),
        }
    }

    /// Verify an entity against GLEIF by name search
    ///
    /// This is a stub implementation.
    pub async fn verify_gleif_by_name(
        &self,
        entity_id: Uuid,
        name: &str,
        jurisdiction: Option<&str>,
    ) -> RegistryCheckResult {
        // STUB: In production, this would call:
        // GET https://api.gleif.org/api/v1/lei-records?filter[entity.legalName]={name}

        RegistryCheckResult {
            entity_id,
            registry: RegistryType::Gleif,
            found: false,
            matches: None,
            confidence: 0.0,
            field_results: vec![FieldCheckResult {
                field: "legal_name".to_string(),
                claimed_value: Some(name.to_string()),
                registry_value: None,
                matches: false,
                match_type: FieldMatchType::NotAvailable,
            }],
            registry_data: None,
            checked_at: Utc::now(),
            error: Some("GLEIF API integration not implemented - stub only".to_string()),
        }
    }

    /// Verify against UK Companies House
    ///
    /// This is a stub implementation.
    pub async fn verify_companies_house_uk(
        &self,
        entity_id: Uuid,
        company_number: &str,
        claimed_name: Option<&str>,
    ) -> RegistryCheckResult {
        // STUB: Would call Companies House API
        // GET https://api.company-information.service.gov.uk/company/{number}

        RegistryCheckResult {
            entity_id,
            registry: RegistryType::CompaniesHouseUk,
            found: false,
            matches: None,
            confidence: 0.0,
            field_results: vec![FieldCheckResult {
                field: "company_number".to_string(),
                claimed_value: Some(company_number.to_string()),
                registry_value: None,
                matches: false,
                match_type: FieldMatchType::NotAvailable,
            }],
            registry_data: None,
            checked_at: Utc::now(),
            error: Some("Companies House API integration not implemented - stub only".to_string()),
        }
    }

    /// Compare two strings for matching with fuzzy tolerance
    pub fn compare_strings(&self, claimed: &str, registry: &str) -> FieldMatchType {
        let claimed_norm = self.normalize_string(claimed);
        let registry_norm = self.normalize_string(registry);

        if claimed_norm == registry_norm {
            FieldMatchType::ExactMatch
        } else if self.levenshtein_distance(&claimed_norm, &registry_norm) <= 2 {
            FieldMatchType::MinorVariation
        } else if self.contains_match(&claimed_norm, &registry_norm) {
            FieldMatchType::FuzzyMatch
        } else {
            FieldMatchType::Mismatch
        }
    }

    /// Normalize a string for comparison
    fn normalize_string(&self, s: &str) -> String {
        s.to_uppercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Calculate Levenshtein edit distance
    fn levenshtein_distance(&self, a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let m = a_chars.len();
        let n = b_chars.len();

        if m == 0 {
            return n;
        }
        if n == 0 {
            return m;
        }

        let mut dp = vec![vec![0; n + 1]; m + 1];

        for i in 0..=m {
            dp[i][0] = i;
        }
        for j in 0..=n {
            dp[0][j] = j;
        }

        for i in 1..=m {
            for j in 1..=n {
                let cost = if a_chars[i - 1] == b_chars[j - 1] {
                    0
                } else {
                    1
                };
                dp[i][j] = (dp[i - 1][j] + 1)
                    .min(dp[i][j - 1] + 1)
                    .min(dp[i - 1][j - 1] + cost);
            }
        }

        dp[m][n]
    }

    /// Check if one string contains the other (for fuzzy matching)
    fn contains_match(&self, a: &str, b: &str) -> bool {
        a.contains(b) || b.contains(a)
    }
}

impl Default for RegistryVerifier {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_string() {
        let verifier = RegistryVerifier::new();

        assert_eq!(
            verifier.normalize_string("Acme Corp, Inc."),
            "ACME CORP INC"
        );
        assert_eq!(verifier.normalize_string("  ACME   CORP  "), "ACME CORP");
    }

    #[test]
    fn test_string_comparison() {
        let verifier = RegistryVerifier::new();

        // Exact match
        assert_eq!(
            verifier.compare_strings("Acme Corp", "ACME CORP"),
            FieldMatchType::ExactMatch
        );

        // Minor variation (1-2 character difference)
        assert_eq!(
            verifier.compare_strings("Acme Corp", "Acme Crop"),
            FieldMatchType::MinorVariation
        );

        // Mismatch
        assert_eq!(
            verifier.compare_strings("Acme Corp", "Beta Industries"),
            FieldMatchType::Mismatch
        );
    }

    #[test]
    fn test_levenshtein_distance() {
        let verifier = RegistryVerifier::new();

        assert_eq!(verifier.levenshtein_distance("kitten", "sitting"), 3);
        assert_eq!(verifier.levenshtein_distance("", "abc"), 3);
        assert_eq!(verifier.levenshtein_distance("abc", "abc"), 0);
    }

    #[tokio::test]
    async fn test_gleif_lei_validation() {
        let verifier = RegistryVerifier::new();
        let entity_id = Uuid::new_v4();

        // Invalid LEI format
        let result = verifier
            .verify_gleif_by_lei(entity_id, "invalid", None, None)
            .await;
        assert!(!result.found);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("Invalid LEI format"));

        // Valid format but stub returns not found
        let result = verifier
            .verify_gleif_by_lei(entity_id, "5299005GGPDLFQ2CWH92", None, None)
            .await;
        assert!(!result.found);
        assert!(result.error.unwrap().contains("stub only"));
    }
}
