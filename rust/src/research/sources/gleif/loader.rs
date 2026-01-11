//! GLEIF SourceLoader implementation
//!
//! Adapts `GleifClient` to the `SourceLoader` trait.

use super::normalize::{normalize_lei_record, normalize_relationship};
use crate::gleif::GleifClient;
use crate::research::sources::normalized::{
    NormalizedControlHolder, NormalizedEntity, NormalizedOfficer, NormalizedRelationship,
};
use crate::research::sources::traits::{
    FetchControlHoldersOptions, FetchOfficersOptions, FetchOptions, FetchParentChainOptions,
    SearchCandidate, SearchOptions, SourceDataType, SourceLoader,
};
use anyhow::Result;
use async_trait::async_trait;

/// GLEIF source loader
///
/// Provides access to the GLEIF LEI database for entity lookup and
/// corporate hierarchy traversal.
pub struct GleifLoader {
    client: GleifClient,
}

impl GleifLoader {
    /// Create a new GLEIF loader
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: GleifClient::new()?,
        })
    }

    /// Create with an existing client (for testing or shared client)
    pub fn with_client(client: GleifClient) -> Self {
        Self { client }
    }
}

impl Default for GleifLoader {
    fn default() -> Self {
        Self::new().expect("Failed to create GLEIF loader")
    }
}

#[async_trait]
impl SourceLoader for GleifLoader {
    fn source_id(&self) -> &'static str {
        "gleif"
    }

    fn source_name(&self) -> &'static str {
        "GLEIF - Global LEI Foundation"
    }

    fn jurisdictions(&self) -> &[&'static str] {
        &["*"] // Global coverage
    }

    fn provides(&self) -> &[SourceDataType] {
        &[SourceDataType::Entity, SourceDataType::ParentChain]
    }

    fn key_type(&self) -> &'static str {
        "LEI"
    }

    fn validate_key(&self, key: &str) -> bool {
        // LEI is exactly 20 alphanumeric characters
        key.len() == 20 && key.chars().all(|c| c.is_ascii_alphanumeric())
    }

    async fn search(
        &self,
        query: &str,
        options: Option<SearchOptions>,
    ) -> Result<Vec<SearchCandidate>> {
        let opts = options.unwrap_or_default();
        let limit = opts.limit.unwrap_or(20);

        let results = self.client.search_by_name(query, limit).await?;

        let candidates: Vec<SearchCandidate> = results
            .into_iter()
            .filter(|r| {
                // Apply jurisdiction filter if specified
                if let Some(ref jur) = opts.jurisdiction {
                    r.attributes
                        .entity
                        .jurisdiction
                        .as_deref()
                        .is_some_and(|j| j.eq_ignore_ascii_case(jur))
                } else {
                    true
                }
            })
            .filter(|r| {
                // Filter inactive unless requested
                if opts.include_inactive {
                    true
                } else {
                    r.entity_status().is_active()
                }
            })
            .map(|r| {
                let score = calculate_match_score(&r.attributes.entity.legal_name.name, query);
                SearchCandidate {
                    key: r.lei().to_string(),
                    name: r.attributes.entity.legal_name.name.clone(),
                    jurisdiction: r.attributes.entity.jurisdiction.clone(),
                    status: r.attributes.entity.status.clone(),
                    score,
                    metadata: serde_json::json!({
                        "category": r.attributes.entity.category,
                        "registration_status": r.attributes.registration.status,
                        "legal_form": r.attributes.entity.legal_form.as_ref().and_then(|lf| lf.id.clone()),
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
        let record = self.client.get_lei_record(key).await?;
        Ok(normalize_lei_record(&record, opts.include_raw))
    }

    async fn fetch_control_holders(
        &self,
        _key: &str,
        _options: Option<FetchControlHoldersOptions>,
    ) -> Result<Vec<NormalizedControlHolder>> {
        // GLEIF doesn't provide shareholder/control holder data
        Ok(vec![])
    }

    async fn fetch_officers(
        &self,
        _key: &str,
        _options: Option<FetchOfficersOptions>,
    ) -> Result<Vec<NormalizedOfficer>> {
        // GLEIF doesn't provide officer data
        Ok(vec![])
    }

    async fn fetch_parent_chain(
        &self,
        key: &str,
        options: Option<FetchParentChainOptions>,
    ) -> Result<Vec<NormalizedRelationship>> {
        let opts = options.unwrap_or_default();
        let max_depth = opts.max_depth.unwrap_or(10);

        let mut relationships = Vec::new();
        let mut current_lei = key.to_string();
        let mut depth = 0;

        // Get the starting entity name
        let start_record = self.client.get_lei_record(key).await?;
        let mut child_name = start_record.legal_name().to_string();

        while depth < max_depth {
            // Try to get direct parent
            let parent_rel = self.client.get_direct_parent(&current_lei).await?;

            match parent_rel {
                Some(rel) => {
                    // Fetch parent entity to get its name
                    let parent_lei = &rel.attributes.relationship.end_node.id;
                    let parent_record = self.client.get_lei_record(parent_lei).await?;
                    let parent_name = parent_record.legal_name().to_string();

                    relationships.push(normalize_relationship(&rel, &child_name, &parent_name));

                    // Move up the chain
                    current_lei = parent_lei.clone();
                    child_name = parent_name;
                    depth += 1;
                }
                None => {
                    // No more parents - check for ultimate parent if we haven't found any
                    if relationships.is_empty() {
                        if let Some(ult_rel) = self.client.get_ultimate_parent(key).await? {
                            let parent_lei = &ult_rel.attributes.relationship.end_node.id;
                            let parent_record = self.client.get_lei_record(parent_lei).await?;
                            let parent_name = parent_record.legal_name().to_string();

                            relationships.push(normalize_relationship(
                                &ult_rel,
                                &child_name,
                                &parent_name,
                            ));
                        }
                    }
                    break;
                }
            }
        }

        Ok(relationships)
    }
}

/// Calculate a simple match score between a result name and query
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

    // Word-level matching
    let name_words: std::collections::HashSet<_> = name_lower.split_whitespace().collect();
    let query_words: std::collections::HashSet<_> = query_lower.split_whitespace().collect();

    if query_words.is_empty() {
        return 0.0;
    }

    let matching = name_words.intersection(&query_words).count();
    let total = query_words.len();

    (matching as f64) / (total as f64) * 0.6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_key() {
        let loader = GleifLoader::default();

        // Valid LEIs
        assert!(loader.validate_key("5493001KJTIIGC8Y1R12"));
        assert!(loader.validate_key("529900K9B0N5BT694847"));

        // Invalid LEIs
        assert!(!loader.validate_key("too_short"));
        assert!(!loader.validate_key("5493001KJTIIGC8Y1R12X")); // Too long
        assert!(!loader.validate_key("5493001KJTIIGC8Y1R1-")); // Invalid char
    }

    #[test]
    fn test_calculate_match_score() {
        assert_eq!(calculate_match_score("Acme Corp", "Acme Corp"), 1.0);
        assert_eq!(calculate_match_score("ACME CORP", "acme corp"), 1.0);
        assert!(calculate_match_score("Acme Corporation", "Acme") > 0.8);
        assert!(calculate_match_score("The Acme Company", "Acme") > 0.5);
        assert!(calculate_match_score("Unrelated Inc", "Acme") < 0.1);
    }

    #[test]
    fn test_source_metadata() {
        let loader = GleifLoader::default();

        assert_eq!(loader.source_id(), "gleif");
        assert_eq!(loader.key_type(), "LEI");
        assert_eq!(loader.jurisdictions(), &["*"]);
        assert!(loader.provides().contains(&SourceDataType::Entity));
        assert!(loader.provides().contains(&SourceDataType::ParentChain));
        assert!(!loader.provides().contains(&SourceDataType::ControlHolders));
    }
}
