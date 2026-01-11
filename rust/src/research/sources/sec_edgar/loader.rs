//! SEC EDGAR SourceLoader implementation

use super::client::SecEdgarClient;
use super::types::SecAddress;
use crate::research::sources::normalized::{
    EntityStatus, EntityType, HolderType, NormalizedAddress, NormalizedControlHolder,
    NormalizedEntity, NormalizedOfficer, NormalizedRelationship,
};
use crate::research::sources::traits::{
    FetchControlHoldersOptions, FetchOfficersOptions, FetchOptions, FetchParentChainOptions,
    SearchCandidate, SearchOptions, SourceDataType, SourceLoader,
};
use anyhow::{anyhow, Result};
use async_trait::async_trait;

/// SEC EDGAR source loader
pub struct SecEdgarLoader {
    client: SecEdgarClient,
}

impl SecEdgarLoader {
    /// Create a new loader
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: SecEdgarClient::new()?,
        })
    }

    /// Create with an existing client
    pub fn with_client(client: SecEdgarClient) -> Self {
        Self { client }
    }
}

impl Default for SecEdgarLoader {
    fn default() -> Self {
        Self::new().expect("Failed to create SEC EDGAR loader")
    }
}

#[async_trait]
impl SourceLoader for SecEdgarLoader {
    fn source_id(&self) -> &'static str {
        "sec-edgar"
    }

    fn source_name(&self) -> &'static str {
        "US SEC EDGAR"
    }

    fn jurisdictions(&self) -> &[&'static str] {
        &["US"]
    }

    fn provides(&self) -> &[SourceDataType] {
        &[
            SourceDataType::Entity,
            SourceDataType::ControlHolders,
            SourceDataType::Filings,
        ]
    }

    fn key_type(&self) -> &'static str {
        "CIK"
    }

    fn validate_key(&self, key: &str) -> bool {
        // CIK is up to 10 digits
        let digits_only = key.trim().trim_start_matches('0');
        !digits_only.is_empty()
            && digits_only.len() <= 10
            && digits_only.chars().all(|c| c.is_ascii_digit())
    }

    async fn search(
        &self,
        _query: &str,
        _options: Option<SearchOptions>,
    ) -> Result<Vec<SearchCandidate>> {
        // SEC doesn't have a good company search API via data.sec.gov
        // Would need to use full-text search or company tickers endpoint
        // For now, return empty - users should use CIK directly
        Err(anyhow!(
            "SEC EDGAR search requires CIK or ticker. Use fetch_entity with CIK."
        ))
    }

    async fn fetch_entity(
        &self,
        key: &str,
        options: Option<FetchOptions>,
    ) -> Result<NormalizedEntity> {
        let opts = options.unwrap_or_default();
        let submissions = self.client.get_company(key).await?;

        let business_address = submissions
            .addresses
            .as_ref()
            .and_then(|a| a.business.as_ref())
            .map(normalize_address);

        let entity = NormalizedEntity {
            source_key: submissions.cik_padded(),
            source_name: "SEC EDGAR".to_string(),
            name: submissions.name.clone(),
            lei: None,
            registration_number: Some(submissions.cik_padded()),
            tax_id: submissions.ein.clone(),
            entity_type: Some(map_entity_type(&submissions.entity_type)),
            jurisdiction: Some("US".to_string()),
            status: Some(EntityStatus::Active), // SEC filers are generally active
            incorporated_date: None,            // Not provided by SEC
            dissolved_date: None,
            registered_address: business_address.clone(),
            business_address,
            raw_response: if opts.include_raw {
                serde_json::to_value(&submissions).ok()
            } else {
                None
            },
        };

        Ok(entity)
    }

    async fn fetch_control_holders(
        &self,
        key: &str,
        options: Option<FetchControlHoldersOptions>,
    ) -> Result<Vec<NormalizedControlHolder>> {
        let opts = options.unwrap_or_default();
        let filings = self.client.get_beneficial_ownership_filings(key).await?;

        // Note: Full 13D/13G parsing is complex (semi-structured XML/SGML)
        // This provides basic filing metadata; full parsing would require
        // fetching and parsing each document

        let mut holders = Vec::new();

        // Limit to recent filings
        let limit = 20;
        for filing in filings.into_iter().take(limit) {
            // Skip amendments unless requested
            if !opts.include_ceased && filing.is_amendment() {
                continue;
            }

            // Create a placeholder holder from the filing
            // Full implementation would parse the actual document
            holders.push(NormalizedControlHolder {
                holder_name: format!("13D/G Filer ({})", filing.accession_number),
                holder_type: HolderType::Unknown,
                registration_number: None,
                jurisdiction: None,
                lei: None,
                nationality: None,
                country_of_residence: None,
                date_of_birth_partial: None,
                ownership_pct_low: Some(rust_decimal::Decimal::from(5)), // 13D/G threshold
                ownership_pct_high: None,
                ownership_pct_exact: None,
                voting_pct: None,
                has_voting_rights: true,
                has_appointment_rights: false,
                has_veto_rights: false,
                natures_of_control: vec![filing.form.clone()],
                notified_on: parse_sec_date(&filing.filing_date),
                ceased_on: None,
                source_document: Some(filing.accession_number.clone()),
            });
        }

        Ok(holders)
    }

    async fn fetch_officers(
        &self,
        _key: &str,
        _options: Option<FetchOfficersOptions>,
    ) -> Result<Vec<NormalizedOfficer>> {
        // SEC doesn't provide officer data via the submissions API
        // Would need to parse DEF 14A proxy statements
        Ok(vec![])
    }

    async fn fetch_parent_chain(
        &self,
        _key: &str,
        _options: Option<FetchParentChainOptions>,
    ) -> Result<Vec<NormalizedRelationship>> {
        // SEC doesn't provide parent chain data
        Ok(vec![])
    }
}

/// Map SEC entity type to our EntityType
fn map_entity_type(entity_type: &Option<String>) -> EntityType {
    match entity_type.as_deref().map(|s| s.to_lowercase()).as_deref() {
        Some("operating") => EntityType::LimitedCompany,
        Some("investment") => EntityType::Fund,
        None => EntityType::Unknown("UNSPECIFIED".to_string()),
        Some(other) => {
            tracing::warn!(
                source = "sec-edgar",
                field = "entityType",
                value = other,
                "Unknown SEC entity type"
            );
            EntityType::Unknown(other.to_string())
        }
    }
}

/// Normalize SEC address
fn normalize_address(addr: &SecAddress) -> NormalizedAddress {
    let mut lines = Vec::new();

    if let Some(ref street1) = addr.street1 {
        if !street1.is_empty() {
            lines.push(street1.clone());
        }
    }
    if let Some(ref street2) = addr.street2 {
        if !street2.is_empty() {
            lines.push(street2.clone());
        }
    }

    NormalizedAddress {
        lines,
        city: addr.city.clone(),
        region: addr.state_or_country.clone(),
        postal_code: addr.zip_code.clone(),
        country: Some("US".to_string()),
    }
}

/// Parse SEC date format (YYYY-MM-DD)
fn parse_sec_date(date_str: &str) -> Option<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_key() {
        let loader = SecEdgarLoader::default();

        // Valid CIKs
        assert!(loader.validate_key("320193"));
        assert!(loader.validate_key("0000320193"));
        assert!(loader.validate_key("1"));
        assert!(loader.validate_key("1234567890"));

        // Invalid CIKs
        assert!(!loader.validate_key("")); // Empty
        assert!(!loader.validate_key("0000000000")); // All zeros
        assert!(!loader.validate_key("12345678901")); // Too long
        assert!(!loader.validate_key("AAPL")); // Not numeric
    }

    #[test]
    fn test_source_metadata() {
        let loader = SecEdgarLoader::default();

        assert_eq!(loader.source_id(), "sec-edgar");
        assert_eq!(loader.key_type(), "CIK");
        assert_eq!(loader.jurisdictions(), &["US"]);
        assert!(loader.provides().contains(&SourceDataType::Entity));
        assert!(loader.provides().contains(&SourceDataType::ControlHolders));
        assert!(!loader.provides().contains(&SourceDataType::Officers));
    }

    #[test]
    fn test_map_entity_type() {
        assert!(matches!(
            map_entity_type(&Some("operating".to_string())),
            EntityType::LimitedCompany
        ));
        assert!(matches!(
            map_entity_type(&Some("investment".to_string())),
            EntityType::Fund
        ));
        assert!(matches!(map_entity_type(&None), EntityType::Unknown(_)));
    }
}
