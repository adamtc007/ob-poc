//! GLEIF API Client
//!
//! Rate-limited HTTP client for fetching LEI records and relationship data from the GLEIF API.

use super::types::*;
use anyhow::{Context, Result};
use reqwest::Client;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::time::sleep;

const GLEIF_API_BASE: &str = "https://api.gleif.org/api/v1";
const RATE_LIMIT_DELAY_MS: u64 = 200; // 5 req/sec to be safe

pub struct GleifClient {
    client: Client,
    last_request: Mutex<Instant>,
}

impl GleifClient {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            last_request: Mutex::new(Instant::now()),
        })
    }

    /// Enforce rate limiting between requests
    async fn rate_limit(&self) {
        let elapsed = {
            let last = self.last_request.lock().unwrap();
            last.elapsed()
        };

        if elapsed < Duration::from_millis(RATE_LIMIT_DELAY_MS) {
            sleep(Duration::from_millis(RATE_LIMIT_DELAY_MS) - elapsed).await;
        }

        let mut last = self.last_request.lock().unwrap();
        *last = Instant::now();
    }

    /// Fetch a single LEI record by LEI
    pub async fn get_lei_record(&self, lei: &str) -> Result<LeiRecord> {
        self.rate_limit().await;
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
        self.rate_limit().await;
        let url = format!(
            "{}/lei-records?filter[entity.legalName]={}&page[size]={}",
            GLEIF_API_BASE,
            name.replace(' ', "%20").replace('&', "%26"),
            limit
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to search LEI records")?;

        let text = response
            .text()
            .await
            .context("Failed to read response body")?;

        let parsed: GleifResponse<Vec<LeiRecord>> =
            serde_json::from_str(&text).with_context(|| {
                // Try to get a more specific error by parsing as Value and then trying each record
                let err_msg = match serde_json::from_str::<serde_json::Value>(&text) {
                    Ok(value) => {
                        // First try parsing all records one by one
                        let mut records_ok = true;
                        let mut record_errors = Vec::new();
                        if let Some(data) = value.get("data").and_then(|d| d.as_array()) {
                            for (i, record) in data.iter().enumerate() {
                                if let Err(e) = serde_json::from_value::<LeiRecord>(record.clone())
                                {
                                    records_ok = false;
                                    record_errors.push(format!(
                                        "Record {} error: {} (lei: {})",
                                        i,
                                        e,
                                        record.get("id").and_then(|v| v.as_str()).unwrap_or("?")
                                    ));
                                    if record_errors.len() >= 3 {
                                        break; // Stop at 3 errors
                                    }
                                }
                            }
                        }
                        if records_ok {
                            // All records parse fine - try the meta and links
                            let meta_ok = value
                                .get("meta")
                                .map(|m| serde_json::from_value::<ResponseMeta>(m.clone()).is_ok())
                                .unwrap_or(true);
                            let links_ok = value
                                .get("links")
                                .map(|l| {
                                    serde_json::from_value::<PaginationLinks>(l.clone()).is_ok()
                                })
                                .unwrap_or(true);
                            if !meta_ok {
                                format!(
                                    "Meta parse error. meta: {}",
                                    value
                                        .get("meta")
                                        .map(|v| v.to_string())
                                        .unwrap_or_default()
                                )
                            } else if !links_ok {
                                format!(
                                    "Links parse error. links: {}",
                                    value
                                        .get("links")
                                        .map(|v| v.to_string())
                                        .unwrap_or_default()
                                )
                            } else {
                                "All parts parse individually but combined fails - check struct layout".to_string()
                            }
                        } else {
                            record_errors.join("; ")
                        }
                    }
                    Err(e) => format!(
                        "JSON parse error at line {} col {}: {}",
                        e.line(),
                        e.column(),
                        e
                    ),
                };
                format!(
                    "Failed to parse search response. {}. First 500 chars: {}",
                    err_msg,
                    &text[..text.len().min(500)]
                )
            })?;

        Ok(parsed.data)
    }

    /// Fetch direct parent relationship record
    pub async fn get_direct_parent(&self, lei: &str) -> Result<Option<RelationshipRecord>> {
        self.rate_limit().await;
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
        self.rate_limit().await;
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
        self.rate_limit().await;
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
        self.rate_limit().await;
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
                                queue.push((child.lei().to_string(), depth + 1));
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

    /// Fetch all funds managed by a given fund manager LEI
    /// Uses the GLEIF relationship endpoint: /{manager_lei}/managed-funds
    pub async fn get_managed_funds(&self, manager_lei: &str) -> Result<Vec<LeiRecord>> {
        let mut all_funds = Vec::new();
        let mut page = 1;
        let page_size = 100;

        loop {
            self.rate_limit().await;
            // Use the managed-funds relationship endpoint (correct GLEIF API path)
            let url = format!(
                "{}/lei-records/{}/managed-funds?page%5Bnumber%5D={}&page%5Bsize%5D={}",
                GLEIF_API_BASE, manager_lei, page, page_size
            );

            let response = self.client.get(&url).send().await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(anyhow::anyhow!(
                    "GLEIF API error {}: {}",
                    status,
                    body.chars().take(200).collect::<String>()
                ));
            }

            let text = response.text().await?;
            let data: GleifResponse<Vec<LeiRecord>> =
                serde_json::from_str(&text).with_context(|| {
                    // Try to identify specific parse failures
                    let err_msg = match serde_json::from_str::<serde_json::Value>(&text) {
                        Ok(value) => {
                            let mut record_errors = Vec::new();
                            if let Some(data) = value.get("data").and_then(|d| d.as_array()) {
                                for (i, record) in data.iter().enumerate() {
                                    if let Err(e) =
                                        serde_json::from_value::<LeiRecord>(record.clone())
                                    {
                                        record_errors.push(format!(
                                            "Record {} error: {} (lei: {})",
                                            i,
                                            e,
                                            record
                                                .get("id")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("?")
                                        ));
                                        if record_errors.len() >= 3 {
                                            break;
                                        }
                                    }
                                }
                            }
                            if record_errors.is_empty() {
                                "Unknown parse error".to_string()
                            } else {
                                record_errors.join("; ")
                            }
                        }
                        Err(e) => format!("Invalid JSON: {}", e),
                    };
                    format!(
                        "Failed to parse managed funds response. {}. First 500 chars: {}",
                        err_msg,
                        &text[..text.len().min(500)]
                    )
                })?;

            let count = data.data.len();
            all_funds.extend(data.data);

            tracing::debug!(
                "Fetched page {} with {} funds (total: {})",
                page,
                count,
                all_funds.len()
            );

            // Check if there are more pages
            if count < page_size {
                break;
            }

            // Check pagination info
            if let Some(ref meta) = data.meta {
                if let Some(ref pagination) = meta.pagination {
                    if page >= pagination.last_page {
                        break;
                    }
                }
            }

            page += 1;

            // Safety limit to prevent infinite loops
            if page > 50 {
                tracing::warn!(
                    "Reached max pages (50) fetching managed funds for {}",
                    manager_lei
                );
                break;
            }
        }

        Ok(all_funds)
    }

    /// Fetch umbrella fund for a sub-fund (IS_SUBFUND_OF relationship)
    pub async fn get_umbrella_fund(&self, lei: &str) -> Result<Option<LeiRecord>> {
        self.rate_limit().await;

        // First get the fund's relationships to find umbrella
        let record = self.get_lei_record(lei).await?;

        if let Some(ref rels) = record.relationships {
            if let Some(ref umbrella) = rels.umbrella_fund {
                if let Some(ref url) = umbrella.links.related {
                    if let Some(umbrella_lei) = extract_lei_from_url(url) {
                        return Ok(Some(self.get_lei_record(&umbrella_lei).await?));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Fetch fund manager for a fund (IS_FUND-MANAGED_BY relationship)
    pub async fn get_fund_manager(&self, lei: &str) -> Result<Option<LeiRecord>> {
        self.rate_limit().await;

        let record = self.get_lei_record(lei).await?;

        if let Some(ref rels) = record.relationships {
            if let Some(ref manager) = rels.fund_manager {
                if let Some(ref url) = manager.links.related {
                    if let Some(manager_lei) = extract_lei_from_url(url) {
                        return Ok(Some(self.get_lei_record(&manager_lei).await?));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Fetch master fund for a feeder fund (IS_FEEDER_TO relationship)
    pub async fn get_master_fund(&self, lei: &str) -> Result<Option<LeiRecord>> {
        self.rate_limit().await;

        let record = self.get_lei_record(lei).await?;

        if let Some(ref rels) = record.relationships {
            if let Some(ref master) = rels.master_fund {
                if let Some(ref url) = master.links.related {
                    if let Some(master_lei) = extract_lei_from_url(url) {
                        return Ok(Some(self.get_lei_record(&master_lei).await?));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Look up LEI by ISIN (uses GLEIF ISIN-LEI mapping endpoint)
    pub async fn lookup_by_isin(&self, isin: &str) -> Result<Option<LeiRecord>> {
        self.rate_limit().await;

        // GLEIF provides ISIN-LEI mappings via the lei-records endpoint with filter
        let url = format!("{}/lei-records?filter[isin]={}", GLEIF_API_BASE, isin);

        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            return Err(anyhow::anyhow!(
                "GLEIF API error: {} for ISIN {}",
                response.status(),
                isin
            ));
        }

        let text = response.text().await?;
        let data: GleifResponse<Vec<LeiRecord>> =
            serde_json::from_str(&text).context("Failed to parse ISIN lookup response")?;

        // Return the first matching record
        Ok(data.data.into_iter().next())
    }

    /// Check if an LEI has a reporting exception for its parent
    pub async fn get_reporting_exception(
        &self,
        lei: &str,
    ) -> Result<Option<(Option<String>, Option<String>)>> {
        // Fetch the LEI record with full details
        let record = self.get_lei_record(lei).await?;

        // Check for exception in relationships
        if let Some(ref _rels) = record.relationships {
            // TODO: Fetch actual exceptions from relationship-record URLs
            // For now, we return None for both - this is a placeholder
            let direct_exception: Option<String> = None;
            let ultimate_exception: Option<String> = None;

            return Ok(Some((direct_exception, ultimate_exception)));
        }

        Ok(None)
    }

    /// Search for funds by name pattern with category filter
    /// This is a fallback when the managed-funds relationship endpoint returns empty
    pub async fn search_funds_by_name(
        &self,
        name_pattern: &str,
        limit: usize,
    ) -> Result<Vec<LeiRecord>> {
        let mut all_funds = Vec::new();
        let mut page = 1;
        let page_size = 100.min(limit);

        loop {
            self.rate_limit().await;

            // Search for entities with FUND category and name pattern
            // Use wildcard search with the name pattern
            // URL-encode the name pattern (spaces -> %20, & -> %26)
            let encoded_name = name_pattern
                .replace(' ', "%20")
                .replace('&', "%26")
                .replace('+', "%2B");
            let url = format!(
                "{}/lei-records?filter%5Bentity.legalName%5D={}*&filter%5Bentity.category%5D=FUND&page%5Bnumber%5D={}&page%5Bsize%5D={}",
                GLEIF_API_BASE,
                encoded_name,
                page,
                page_size
            );

            let response = self.client.get(&url).send().await?;

            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(anyhow::anyhow!(
                    "GLEIF API error {}: {}",
                    status,
                    body.chars().take(200).collect::<String>()
                ));
            }

            let text = response.text().await?;
            let data: GleifResponse<Vec<LeiRecord>> =
                serde_json::from_str(&text).with_context(|| {
                    format!(
                        "Failed to parse fund search response. First 500 chars: {}",
                        &text[..text.len().min(500)]
                    )
                })?;

            let count = data.data.len();
            all_funds.extend(data.data);

            tracing::debug!(
                "Fetched page {} with {} funds (total: {})",
                page,
                count,
                all_funds.len()
            );

            // Check if we've hit the limit
            if all_funds.len() >= limit {
                all_funds.truncate(limit);
                break;
            }

            // Check if there are more pages
            if count < page_size {
                break;
            }

            // Check pagination info
            if let Some(ref meta) = data.meta {
                if let Some(ref pagination) = meta.pagination {
                    if page >= pagination.last_page {
                        break;
                    }
                }
            }

            page += 1;

            // Safety limit
            if page > 20 {
                tracing::warn!(
                    "Reached max pages (20) searching funds by name pattern {}",
                    name_pattern
                );
                break;
            }
        }

        Ok(all_funds)
    }
}

impl Default for GleifClient {
    fn default() -> Self {
        Self::new().expect("Failed to create default GLEIF client")
    }
}

/// Extract LEI from a GLEIF API URL like "/api/v1/lei-records/5493001KJTIIGC8Y1R12"
pub fn extract_lei_from_url(url: &str) -> Option<String> {
    url.split('/').next_back().map(|s| s.to_string())
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
