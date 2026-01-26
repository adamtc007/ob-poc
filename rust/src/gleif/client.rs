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

/// A discovered parent-child relationship from tree traversal
#[derive(Debug, Clone)]
pub struct DiscoveredRelationship {
    pub child_lei: String,
    pub parent_lei: String,
    pub relationship_type: String,
    /// Category for filtering (OWNERSHIP, INVESTMENT_MANAGEMENT, FUND_STRUCTURE)
    pub relationship_category: super::types::RelationshipCategory,
    /// Whether the child entity is a fund (quick filter for IM relationships)
    pub is_fund: bool,
}

/// Result of fetching a corporate tree - includes both entities and relationships
#[derive(Debug)]
pub struct CorporateTreeResult {
    pub records: Vec<LeiRecord>,
    pub relationships: Vec<DiscoveredRelationship>,
    /// Count of fund entities discovered (subset of records where is_fund=true)
    pub fund_count: usize,
    /// Count of ManCo entities that were expanded for managed funds
    pub mancos_expanded: usize,
}

/// Options for controlling corporate tree traversal behavior
#[derive(Debug, Clone)]
pub struct TreeFetchOptions {
    /// Maximum depth for consolidation hierarchy traversal
    pub max_depth: usize,
    /// Include funds managed by entities in the tree (IS_FUND-MANAGED_BY)
    pub include_managed_funds: bool,
    /// Include umbrella/subfund relationships (IS_SUBFUND_OF)
    pub include_fund_structures: bool,
    /// Include master/feeder relationships (IS_FEEDER_TO)
    pub include_master_feeder: bool,
    /// Filter funds by type (e.g., "UCITS", "AIF") - empty means all
    pub fund_type_filter: Vec<String>,
    /// Filter funds by jurisdiction (e.g., "LU", "IE") - empty means all
    pub fund_jurisdiction_filter: Vec<String>,
    /// Maximum funds to load per ManCo (safety limit to prevent runaway API calls)
    pub max_funds_per_manco: Option<usize>,
}

impl Default for TreeFetchOptions {
    fn default() -> Self {
        Self {
            max_depth: 10,
            include_managed_funds: false,
            include_fund_structures: false,
            include_master_feeder: false,
            fund_type_filter: vec![],
            fund_jurisdiction_filter: vec![],
            max_funds_per_manco: Some(500),
        }
    }
}

impl TreeFetchOptions {
    /// Create options for ownership-only traversal (default, backwards compatible)
    pub fn ownership_only() -> Self {
        Self {
            max_depth: 10,
            include_managed_funds: false,
            include_fund_structures: false,
            include_master_feeder: false,
            fund_type_filter: vec![],
            fund_jurisdiction_filter: vec![],
            max_funds_per_manco: None,
        }
    }

    /// Create options for full traversal including all fund relationships
    pub fn full_with_funds() -> Self {
        Self {
            max_depth: 10,
            include_managed_funds: true,
            include_fund_structures: true,
            include_master_feeder: true,
            fund_type_filter: vec![],
            fund_jurisdiction_filter: vec![],
            max_funds_per_manco: Some(500), // Safety limit
        }
    }

    /// Create options with fund inclusion and custom limits
    pub fn with_fund_limit(max_funds_per_manco: usize) -> Self {
        Self {
            max_depth: 10,
            include_managed_funds: true,
            include_fund_structures: true,
            include_master_feeder: true,
            fund_type_filter: vec![],
            fund_jurisdiction_filter: vec![],
            max_funds_per_manco: Some(max_funds_per_manco),
        }
    }
}

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
        let mut all_children = Vec::new();
        let mut page = 1;
        let page_size = 100;

        loop {
            self.rate_limit().await;
            let url = format!(
                "{}/lei-records/{}/direct-children?page%5Bnumber%5D={}&page%5Bsize%5D={}",
                GLEIF_API_BASE, lei, page, page_size
            );

            let response = self.client.get(&url).send().await?;

            if !response.status().is_success() {
                // 404 means no children, which is fine
                if response.status() == reqwest::StatusCode::NOT_FOUND {
                    break;
                }
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                return Err(anyhow::anyhow!(
                    "GLEIF API error {}: {}",
                    status,
                    body.chars().take(200).collect::<String>()
                ));
            }

            let data: GleifResponse<Vec<LeiRecord>> = response
                .json()
                .await
                .context("Failed to parse children response")?;

            let count = data.data.len();
            all_children.extend(data.data);

            // If we got fewer than page_size, we're done
            if count < page_size {
                break;
            }
            page += 1;
        }

        Ok(all_children)
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
    /// Returns all entities in the tree (parents and children) plus discovered relationships
    pub async fn fetch_corporate_tree(
        &self,
        root_lei: &str,
        max_depth: usize,
    ) -> Result<CorporateTreeResult> {
        let mut all_records = Vec::new();
        let mut all_relationships = Vec::new();
        let mut visited = std::collections::HashSet::new();
        // Queue contains (lei, depth, optional_parent_lei)
        let mut queue = vec![(root_lei.to_string(), 0usize, None::<String>)];

        while let Some((lei, depth, parent_lei)) = queue.pop() {
            if visited.contains(&lei) || depth > max_depth {
                continue;
            }
            visited.insert(lei.clone());

            // If we know the parent (from traversing children), record that relationship
            if let Some(ref parent) = parent_lei {
                all_relationships.push(DiscoveredRelationship {
                    child_lei: lei.clone(),
                    parent_lei: parent.clone(),
                    relationship_type: "DIRECT_PARENT".to_string(),
                    relationship_category: super::types::RelationshipCategory::Ownership,
                    is_fund: false, // Will be updated when we fetch the record
                });
            }

            // Fetch the record
            match self.get_lei_record(&lei).await {
                Ok(record) => {
                    // Queue parents if not at max depth (and record relationship)
                    if depth < max_depth {
                        if let Some(ref rels) = record.relationships {
                            // Check direct parent
                            if let Some(ref dp) = rels.direct_parent {
                                if let Some(ref url) = dp.links.related {
                                    if let Some(extracted_parent_lei) = extract_lei_from_url(url) {
                                        // Record this relationship
                                        all_relationships.push(DiscoveredRelationship {
                                            child_lei: lei.clone(),
                                            parent_lei: extracted_parent_lei.clone(),
                                            relationship_type: "DIRECT_PARENT".to_string(),
                                            relationship_category:
                                                super::types::RelationshipCategory::Ownership,
                                            is_fund: record.is_fund(),
                                        });
                                        queue.push((extracted_parent_lei, depth + 1, None));
                                    }
                                }
                            }
                        }
                    }

                    // Queue children (pass current lei as their parent)
                    if depth < max_depth {
                        if let Ok(children) = self.get_direct_children(&lei).await {
                            for child in children {
                                let child_lei = child.lei().to_string();
                                queue.push((child_lei, depth + 1, Some(lei.clone())));
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

        // Deduplicate relationships (same child-parent pair might be discovered from both directions)
        let mut seen_rels = std::collections::HashSet::new();
        all_relationships.retain(|r| {
            let key = (r.child_lei.clone(), r.parent_lei.clone());
            seen_rels.insert(key)
        });

        // Count funds in the result
        let fund_count = all_records.iter().filter(|r| r.is_fund()).count();

        Ok(CorporateTreeResult {
            records: all_records,
            relationships: all_relationships,
            fund_count,
            mancos_expanded: 0, // No managed funds expansion in basic traversal
        })
    }

    /// Fetch the full corporate tree with optional fund relationship expansion
    ///
    /// This enhanced traversal can optionally load funds managed by entities in the tree
    /// (IS_FUND-MANAGED_BY relationships) and fund structure relationships (IS_SUBFUND_OF,
    /// IS_FEEDER_TO).
    ///
    /// # Arguments
    /// * `root_lei` - The LEI to start traversal from
    /// * `options` - Controls fund loading behavior and limits
    pub async fn fetch_corporate_tree_with_options(
        &self,
        root_lei: &str,
        options: TreeFetchOptions,
    ) -> Result<CorporateTreeResult> {
        use super::types::RelationshipCategory;

        let mut all_records = Vec::new();
        let mut all_relationships = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut mancos_expanded = 0usize;

        // Queue contains (lei, depth, optional_parent_lei, is_from_fund_expansion)
        // is_from_fund_expansion=true means we found this via managed-funds, don't re-expand
        let mut queue = vec![(root_lei.to_string(), 0usize, None::<String>, false)];

        while let Some((lei, depth, parent_lei, is_from_fund_expansion)) = queue.pop() {
            if visited.contains(&lei) || depth > options.max_depth {
                continue;
            }
            visited.insert(lei.clone());

            // If we know the parent (from traversing children), record that relationship
            if let Some(ref parent) = parent_lei {
                all_relationships.push(DiscoveredRelationship {
                    child_lei: lei.clone(),
                    parent_lei: parent.clone(),
                    relationship_type: "DIRECT_PARENT".to_string(),
                    relationship_category: RelationshipCategory::Ownership,
                    is_fund: false, // Will be updated when we fetch the record
                });
            }

            // Fetch the record
            match self.get_lei_record(&lei).await {
                Ok(record) => {
                    let record_is_fund = record.is_fund();

                    // Queue parents if not at max depth (and record relationship)
                    if depth < options.max_depth {
                        if let Some(ref rels) = record.relationships {
                            // Check direct parent
                            if let Some(ref dp) = rels.direct_parent {
                                if let Some(ref url) = dp.links.related {
                                    if let Some(extracted_parent_lei) = extract_lei_from_url(url) {
                                        all_relationships.push(DiscoveredRelationship {
                                            child_lei: lei.clone(),
                                            parent_lei: extracted_parent_lei.clone(),
                                            relationship_type: "DIRECT_PARENT".to_string(),
                                            relationship_category: RelationshipCategory::Ownership,
                                            is_fund: record_is_fund,
                                        });
                                        queue.push((extracted_parent_lei, depth + 1, None, false));
                                    }
                                }
                            }
                        }
                    }

                    // Queue children (pass current lei as their parent)
                    if depth < options.max_depth {
                        if let Ok(children) = self.get_direct_children(&lei).await {
                            for child in children {
                                let child_lei = child.lei().to_string();
                                queue.push((child_lei, depth + 1, Some(lei.clone()), false));
                            }
                        }
                    }

                    // === FUND EXPANSION: Check if entity manages funds ===
                    if options.include_managed_funds && !is_from_fund_expansion {
                        if let Some(ref rels) = record.relationships {
                            if rels.managed_funds.is_some() {
                                match self.get_managed_funds(&lei).await {
                                    Ok(funds) => {
                                        let total_fund_count = funds.len();
                                        tracing::info!(
                                            lei = %lei,
                                            fund_count = total_fund_count,
                                            "Found ManCo with managed funds"
                                        );
                                        mancos_expanded += 1;

                                        for (idx, fund) in funds.into_iter().enumerate() {
                                            // Apply safety limit
                                            if let Some(max) = options.max_funds_per_manco {
                                                if idx >= max {
                                                    tracing::warn!(
                                                        lei = %lei,
                                                        total = total_fund_count,
                                                        loaded = max,
                                                        "Truncated fund loading at limit"
                                                    );
                                                    break;
                                                }
                                            }

                                            let fund_lei = fund.lei().to_string();

                                            // Apply jurisdiction filter if set
                                            if !options.fund_jurisdiction_filter.is_empty() {
                                                if let Some(jur) = fund.jurisdiction() {
                                                    if !options
                                                        .fund_jurisdiction_filter
                                                        .iter()
                                                        .any(|f| f.eq_ignore_ascii_case(jur))
                                                    {
                                                        continue;
                                                    }
                                                }
                                            }

                                            // Record IM relationship (fund -> ManCo)
                                            all_relationships.push(DiscoveredRelationship {
                                                child_lei: fund_lei.clone(),
                                                parent_lei: lei.clone(),
                                                relationship_type: "IS_FUND-MANAGED_BY".to_string(),
                                                relationship_category:
                                                    RelationshipCategory::InvestmentManagement,
                                                is_fund: true,
                                            });

                                            // Queue fund for structure discovery if not already visited
                                            if options.include_fund_structures
                                                && !visited.contains(&fund_lei)
                                            {
                                                // Don't increase depth for IM links - parallel dimension
                                                queue.push((fund_lei.clone(), depth, None, true));
                                            }

                                            // Add fund record if not already visited
                                            if !visited.contains(&fund_lei) {
                                                visited.insert(fund_lei.clone());
                                                all_records.push(fund);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            lei = %lei,
                                            error = %e,
                                            "Failed to fetch managed funds"
                                        );
                                    }
                                }
                            }
                        }
                    }

                    // === FUND STRUCTURE: Check for umbrella fund (IS_SUBFUND_OF) ===
                    if options.include_fund_structures && record_is_fund {
                        if let Some(ref rels) = record.relationships {
                            if let Some(ref umbrella) = rels.umbrella_fund {
                                if let Some(ref url) = umbrella.links.related {
                                    if let Some(umbrella_lei) = extract_lei_from_url(url) {
                                        all_relationships.push(DiscoveredRelationship {
                                            child_lei: lei.clone(),
                                            parent_lei: umbrella_lei.clone(),
                                            relationship_type: "IS_SUBFUND_OF".to_string(),
                                            relationship_category:
                                                RelationshipCategory::FundStructure,
                                            is_fund: true,
                                        });

                                        if !visited.contains(&umbrella_lei) {
                                            queue.push((umbrella_lei, depth, None, true));
                                        }
                                    }
                                }
                            }

                            // === MASTER-FEEDER: Check for master fund (IS_FEEDER_TO) ===
                            if options.include_master_feeder {
                                if let Some(ref master) = rels.master_fund {
                                    if let Some(ref url) = master.links.related {
                                        if let Some(master_lei) = extract_lei_from_url(url) {
                                            all_relationships.push(DiscoveredRelationship {
                                                child_lei: lei.clone(),
                                                parent_lei: master_lei.clone(),
                                                relationship_type: "IS_FEEDER_TO".to_string(),
                                                relationship_category:
                                                    RelationshipCategory::FundStructure,
                                                is_fund: true,
                                            });

                                            if !visited.contains(&master_lei) {
                                                queue.push((master_lei, depth, None, true));
                                            }
                                        }
                                    }
                                }
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

        // Deduplicate relationships
        let mut seen_rels = std::collections::HashSet::new();
        all_relationships.retain(|r| {
            let key = (
                r.child_lei.clone(),
                r.parent_lei.clone(),
                r.relationship_type.clone(),
            );
            seen_rels.insert(key)
        });

        // Count funds in the result
        let fund_count = all_records.iter().filter(|r| r.is_fund()).count();

        Ok(CorporateTreeResult {
            records: all_records,
            relationships: all_relationships,
            fund_count,
            mancos_expanded,
        })
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
