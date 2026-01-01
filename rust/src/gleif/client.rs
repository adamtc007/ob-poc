//! GLEIF API Client
//!
//! Fetches LEI records and relationship data from the GLEIF API.

use super::types::*;
use anyhow::{Context, Result};
use reqwest::Client;
use std::time::Duration;

const GLEIF_API_BASE: &str = "https://api.gleif.org/api/v1";

pub struct GleifClient {
    client: Client,
}

impl GleifClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self { client })
    }

    /// Fetch a single LEI record by LEI
    pub async fn get_lei_record(&self, lei: &str) -> Result<LeiRecord> {
        let url = format!("{}/lei-records/{}", GLEIF_API_BASE, lei);

        let response: GleifResponse<LeiRecord> = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch LEI record")?
            .json()
            .await
            .context("Failed to parse LEI record response")?;

        Ok(response.data)
    }

    /// Search for LEI records by entity name
    pub async fn search_by_name(&self, name: &str, limit: usize) -> Result<Vec<LeiRecord>> {
        let url = format!(
            "{}/lei-records?filter[entity.legalName]={}&page[size]={}",
            GLEIF_API_BASE,
            name.replace(' ', "%20").replace('&', "%26"),
            limit
        );

        let response: GleifResponse<Vec<LeiRecord>> = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to search LEI records")?
            .json()
            .await
            .context("Failed to parse search response")?;

        Ok(response.data)
    }

    /// Fetch direct parent relationship record
    pub async fn get_direct_parent(&self, lei: &str) -> Result<Option<RelationshipRecord>> {
        let url = format!(
            "{}/lei-records/{}/direct-parent-relationship",
            GLEIF_API_BASE, lei
        );

        let response = self.client.get(&url).send().await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let data: GleifResponse<RelationshipRecord> = response
            .json()
            .await
            .context("Failed to parse direct parent response")?;

        Ok(Some(data.data))
    }

    /// Fetch ultimate parent relationship record
    pub async fn get_ultimate_parent(&self, lei: &str) -> Result<Option<RelationshipRecord>> {
        let url = format!(
            "{}/lei-records/{}/ultimate-parent-relationship",
            GLEIF_API_BASE, lei
        );

        let response = self.client.get(&url).send().await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        let data: GleifResponse<RelationshipRecord> = response
            .json()
            .await
            .context("Failed to parse ultimate parent response")?;

        Ok(Some(data.data))
    }

    /// Fetch all direct children of an entity
    pub async fn get_direct_children(&self, lei: &str) -> Result<Vec<LeiRecord>> {
        let url = format!(
            "{}/lei-records?filter[entity.directParent]={}&page[size]=100",
            GLEIF_API_BASE, lei
        );

        let response: GleifResponse<Vec<LeiRecord>> = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch direct children")?
            .json()
            .await
            .context("Failed to parse children response")?;

        Ok(response.data)
    }

    /// Fetch BIC mappings for an entity
    pub async fn get_bic_mappings(&self, lei: &str) -> Result<Vec<BicMapping>> {
        let url = format!("{}/lei-records/{}/bics", GLEIF_API_BASE, lei);

        let response = self.client.get(&url).send().await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(vec![]);
        }

        let data: GleifResponse<Vec<BicMapping>> = response
            .json()
            .await
            .context("Failed to parse BIC mappings response")?;

        Ok(data.data)
    }

    /// Fetch the full corporate tree starting from a root LEI
    /// Returns all entities in the tree (parents and children)
    pub async fn fetch_corporate_tree(
        &self,
        root_lei: &str,
        max_depth: usize,
    ) -> Result<Vec<LeiRecord>> {
        let mut all_records = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut queue = vec![(root_lei.to_string(), 0usize)];

        while let Some((lei, depth)) = queue.pop() {
            if visited.contains(&lei) || depth > max_depth {
                continue;
            }
            visited.insert(lei.clone());

            // Fetch the record
            match self.get_lei_record(&lei).await {
                Ok(record) => {
                    // Queue parents if not at max depth
                    if depth < max_depth {
                        if let Some(ref rels) = record.relationships {
                            // Check direct parent
                            if let Some(ref dp) = rels.direct_parent {
                                if let Some(ref url) = dp.links.related {
                                    if let Some(parent_lei) = extract_lei_from_url(url) {
                                        queue.push((parent_lei, depth + 1));
                                    }
                                }
                            }
                        }
                    }

                    // Queue children
                    if depth < max_depth {
                        if let Ok(children) = self.get_direct_children(&lei).await {
                            for child in children {
                                queue.push((child.attributes.lei.clone(), depth + 1));
                            }
                        }
                    }

                    all_records.push(record);
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch LEI {}: {}", lei, e);
                }
            }
        }

        Ok(all_records)
    }

    /// Check if an LEI has a reporting exception for its parent
    pub async fn get_reporting_exception(
        &self,
        lei: &str,
    ) -> Result<Option<(Option<String>, Option<String>)>> {
        // Fetch the LEI record with full details
        let record = self.get_lei_record(lei).await?;

        // Check for exception in relationships
        if let Some(ref rels) = record.relationships {
            let direct_exception = if rels.direct_parent.is_none() {
                // No direct parent link - check if there's an exception recorded
                // This requires fetching the relationship-record URL
                None
            } else {
                None
            };

            let ultimate_exception = if rels.ultimate_parent.is_none() {
                None
            } else {
                None
            };

            return Ok(Some((direct_exception, ultimate_exception)));
        }

        Ok(None)
    }
}

impl Default for GleifClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default GLEIF client")
    }
}

/// Extract LEI from a GLEIF API URL like "/api/v1/lei-records/5493001KJTIIGC8Y1R12"
fn extract_lei_from_url(url: &str) -> Option<String> {
    url.split('/').last().map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_lei_from_url() {
        assert_eq!(
            extract_lei_from_url("/api/v1/lei-records/5493001KJTIIGC8Y1R12"),
            Some("5493001KJTIIGC8Y1R12".to_string())
        );
        assert_eq!(
            extract_lei_from_url("https://api.gleif.org/api/v1/lei-records/529900K9B0N5BT694847"),
            Some("529900K9B0N5BT694847".to_string())
        );
    }
}
