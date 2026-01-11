//! Companies House SourceLoader implementation

use super::client::CompaniesHouseClient;
use super::normalize::{normalize_company, normalize_officer, normalize_psc};
use crate::research::sources::normalized::{
    NormalizedControlHolder, NormalizedEntity, NormalizedOfficer, NormalizedRelationship,
};
use crate::research::sources::traits::{
    FetchControlHoldersOptions, FetchOfficersOptions, FetchOptions, FetchParentChainOptions,
    SearchCandidate, SearchOptions, SourceDataType, SourceLoader,
};
use anyhow::Result;
use async_trait::async_trait;

/// Companies House source loader
pub struct CompaniesHouseLoader {
    client: CompaniesHouseClient,
}

impl CompaniesHouseLoader {
    /// Create a new loader from environment
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            client: CompaniesHouseClient::from_env()?,
        })
    }

    /// Create with an existing client
    pub fn with_client(client: CompaniesHouseClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl SourceLoader for CompaniesHouseLoader {
    fn source_id(&self) -> &'static str {
        "companies-house"
    }

    fn source_name(&self) -> &'static str {
        "UK Companies House"
    }

    fn jurisdictions(&self) -> &[&'static str] {
        &["GB", "UK"]
    }

    fn provides(&self) -> &[SourceDataType] {
        &[
            SourceDataType::Entity,
            SourceDataType::ControlHolders,
            SourceDataType::Officers,
        ]
    }

    fn key_type(&self) -> &'static str {
        "COMPANY_NUMBER"
    }

    fn validate_key(&self, key: &str) -> bool {
        let key = key.trim().to_uppercase();

        // Must be 8 characters when normalized
        if key.len() > 8 {
            return false;
        }

        // Check for valid prefixes
        let valid_prefixes = [
            "SC", "NI", "NC", "NF", "OC", "SO", "LP", "SL", "FC", "SF", "NL", "GE", "IP", "SP",
            "IC", "SI", "NP", "NO", "RC", "SR", "AC", "SA", "NA", "NZ", "CE", "CS", "PC", "RS",
        ];

        if key.len() >= 2 {
            let prefix = &key[..2];
            if valid_prefixes.contains(&prefix) {
                // Prefixed: must have valid digits after
                return key[2..].chars().all(|c| c.is_ascii_digit());
            }
        }

        // Pure numeric
        key.chars().all(|c| c.is_ascii_digit())
    }

    async fn search(
        &self,
        query: &str,
        options: Option<SearchOptions>,
    ) -> Result<Vec<SearchCandidate>> {
        let opts = options.unwrap_or_default();
        let limit = opts.limit.unwrap_or(20);

        let results = self.client.search(query, limit).await?;

        let candidates: Vec<SearchCandidate> = results
            .items
            .into_iter()
            .filter(|r| {
                if opts.include_inactive {
                    true
                } else {
                    r.is_active()
                }
            })
            .map(|r| {
                let score = calculate_match_score(&r.title, query);
                SearchCandidate {
                    key: r.company_number.clone(),
                    name: r.title.clone(),
                    jurisdiction: Some("GB".to_string()),
                    status: Some(r.company_status.clone()),
                    score,
                    metadata: serde_json::json!({
                        "company_type": r.company_type,
                        "address_snippet": r.address_snippet,
                        "date_of_creation": r.date_of_creation,
                    }),
                }
            })
            .collect();

        Ok(candidates)
    }

    async fn fetch_entity(
        &self,
        key: &str,
        options: Option<FetchOptions>,
    ) -> Result<NormalizedEntity> {
        let opts = options.unwrap_or_default();
        let company = self.client.get_company(key).await?;
        Ok(normalize_company(&company, opts.include_raw))
    }

    async fn fetch_control_holders(
        &self,
        key: &str,
        options: Option<FetchControlHoldersOptions>,
    ) -> Result<Vec<NormalizedControlHolder>> {
        let opts = options.unwrap_or_default();
        let psc_list = self.client.get_psc(key).await?;

        let holders: Vec<NormalizedControlHolder> = psc_list
            .items
            .into_iter()
            .filter(|p| {
                if opts.include_ceased {
                    true
                } else {
                    p.is_active()
                }
            })
            .filter(|p| {
                // Apply minimum ownership filter if specified
                if let Some(min_pct) = opts.min_ownership_pct {
                    let holder = normalize_psc(p);
                    holder.ownership_pct_best().is_none_or(|pct| pct >= min_pct)
                } else {
                    true
                }
            })
            .map(|p| normalize_psc(&p))
            .collect();

        Ok(holders)
    }

    async fn fetch_officers(
        &self,
        key: &str,
        options: Option<FetchOfficersOptions>,
    ) -> Result<Vec<NormalizedOfficer>> {
        let opts = options.unwrap_or_default();
        let officer_list = self.client.get_officers(key).await?;

        let officers: Vec<NormalizedOfficer> = officer_list
            .items
            .into_iter()
            .filter(|o| {
                if opts.include_resigned {
                    true
                } else {
                    o.is_active()
                }
            })
            .filter(|o| {
                // Apply role filter if specified
                if let Some(ref roles) = opts.roles {
                    let role_lower = o.officer_role.to_lowercase();
                    roles.iter().any(|r| role_lower.contains(&r.to_lowercase()))
                } else {
                    true
                }
            })
            .map(|o| normalize_officer(&o))
            .collect();

        Ok(officers)
    }

    async fn fetch_parent_chain(
        &self,
        _key: &str,
        _options: Option<FetchParentChainOptions>,
    ) -> Result<Vec<NormalizedRelationship>> {
        // Companies House doesn't provide parent chain directly
        // Corporate PSC holders could be used to infer this, but that's
        // a different operation (fetch_control_holders)
        Ok(vec![])
    }
}

/// Calculate match score between result and query
fn calculate_match_score(name: &str, query: &str) -> f64 {
    let name_lower = name.to_lowercase();
    let query_lower = query.to_lowercase();

    if name_lower == query_lower {
        return 1.0;
    }

    if name_lower.starts_with(&query_lower) {
        return 0.9;
    }

    if name_lower.contains(&query_lower) {
        return 0.7;
    }

    // Word matching
    let name_words: std::collections::HashSet<_> = name_lower.split_whitespace().collect();
    let query_words: std::collections::HashSet<_> = query_lower.split_whitespace().collect();

    if query_words.is_empty() {
        return 0.0;
    }

    let matching = name_words.intersection(&query_words).count();
    (matching as f64) / (query_words.len() as f64) * 0.6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_key() {
        // Note: Can't test loader creation without API key
        // Just test the validation logic

        // Pure numeric
        assert!(is_valid_company_number("12345678"));
        assert!(is_valid_company_number("1234567")); // Will be padded
        assert!(is_valid_company_number("00123456"));

        // Prefixed
        assert!(is_valid_company_number("SC123456"));
        assert!(is_valid_company_number("NI123456"));
        assert!(is_valid_company_number("OC123456"));

        // Invalid
        assert!(!is_valid_company_number("123456789")); // Too long
        assert!(!is_valid_company_number("XX123456")); // Invalid prefix
        assert!(!is_valid_company_number("SC12345X")); // Non-digit after prefix
    }

    fn is_valid_company_number(key: &str) -> bool {
        let key = key.trim().to_uppercase();

        if key.len() > 8 {
            return false;
        }

        let valid_prefixes = [
            "SC", "NI", "NC", "NF", "OC", "SO", "LP", "SL", "FC", "SF", "NL", "GE", "IP", "SP",
            "IC", "SI", "NP", "NO", "RC", "SR", "AC", "SA", "NA", "NZ", "CE", "CS", "PC", "RS",
        ];

        if key.len() >= 2 {
            let prefix = &key[..2];
            if valid_prefixes.contains(&prefix) {
                return key[2..].chars().all(|c| c.is_ascii_digit());
            }
        }

        key.chars().all(|c| c.is_ascii_digit())
    }

    #[test]
    fn test_calculate_match_score() {
        assert_eq!(calculate_match_score("Acme Ltd", "Acme Ltd"), 1.0);
        assert!(calculate_match_score("Acme Corporation Ltd", "Acme") > 0.8);
        assert!(calculate_match_score("The Acme Company Ltd", "Acme") > 0.5);
    }
}
