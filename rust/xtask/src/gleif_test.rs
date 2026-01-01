//! GLEIF API Test Harness
//!
//! Comprehensive testing of the GLEIF API for documentation and edge case discovery.
//!
//! ## Test Coverage
//! 1. API Endpoint Coverage - all GLEIF endpoints we use
//! 2. Response Structure Capture - save raw JSON for documentation
//! 3. Relationship Type Discovery - catalog all relationship types seen
//! 4. Edge Cases - SICAVs, mergers, missing parents, inactive LEIs
//! 5. Rate Limiting - verify our throttling works
//! 6. Output Format - structured output for documentation
//!
//! ## Full Crawl Mode
//! The `--crawl` flag enables a full entity download and import:
//! - Starts from a root LEI (default: Allianz SE)
//! - Recursively fetches all children and managed funds
//! - Walks ownership chains upward to natural persons
//! - Imports all entities via DSL verbs
//! - Creates ownership relationships

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use uuid::Uuid;

// =============================================================================
// Test LEIs - Curated for different scenarios
// =============================================================================

/// Allianz SE (Head Office) - Large corporate with many children
pub const ALLIANZ_SE_LEI: &str = "529900K9B0N5BT694847";

/// Allianz Global Investors GmbH (ManCo) - Fund manager with managed funds
pub const ALLIANZ_GI_LEI: &str = "529900FAHFDMSXCPII15";

/// A known SICAV (self-governing fund structure)
#[allow(dead_code)]
pub const SICAV_EXAMPLE_LEI: &str = "222100CJK3CNESPLY225"; // Allianz Global Investors Fund SICAV

/// A known sub-fund with umbrella relationship
#[allow(dead_code)]
pub const SUBFUND_EXAMPLE_LEI: &str = "529900PJMG24KLZLJT60";

/// An inactive/merged LEI for successor testing
#[allow(dead_code)]
pub const MERGED_LEI_EXAMPLE: &str = "5493006MHB84DD0ZWV18"; // Example - may need updating

/// A feeder fund for master-feeder testing
#[allow(dead_code)]
pub const FEEDER_FUND_LEI: &str = "529900T8BM49AURSDO55"; // Example feeder

// =============================================================================
// Configuration
// =============================================================================

const GLEIF_API_BASE: &str = "https://api.gleif.org/api/v1";
const RATE_LIMIT_DELAY_MS: u64 = 250; // Conservative rate limiting
const OUTPUT_DIR: &str = "data/gleif_test_output";
const PAGE_SIZE: usize = 100;

// =============================================================================
// Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub output_dir: PathBuf,
    pub save_responses: bool,
    pub verbose: bool,
    pub max_managed_funds: usize,
    pub max_children: usize,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from(OUTPUT_DIR),
            save_responses: true,
            verbose: false,
            max_managed_funds: 10,
            max_children: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlConfig {
    pub root_lei: String,
    pub max_depth: usize,
    pub max_entities: usize,
    pub crawl_children: bool,
    pub crawl_managed_funds: bool,
    pub crawl_parents: bool,
    pub dry_run: bool,
    pub verbose: bool,
    pub output_dir: PathBuf,
}

impl Default for CrawlConfig {
    fn default() -> Self {
        Self {
            root_lei: ALLIANZ_SE_LEI.to_string(),
            max_depth: 5,
            max_entities: 1000,
            crawl_children: true,
            crawl_managed_funds: true,
            crawl_parents: true,
            dry_run: false,
            verbose: false,
            output_dir: PathBuf::from(OUTPUT_DIR),
        }
    }
}

#[derive(Debug, Default, Serialize)]
pub struct TestReport {
    pub endpoints_tested: Vec<EndpointResult>,
    pub relationship_types: HashSet<String>,
    pub entity_categories: HashSet<String>,
    pub entity_statuses: HashSet<String>,
    pub legal_forms: HashSet<String>,
    pub edge_cases: Vec<EdgeCaseResult>,
    pub rate_limit_stats: RateLimitStats,
    pub errors: Vec<String>,
    pub summary: TestSummary,
}

#[derive(Debug, Clone, Serialize)]
pub struct EndpointResult {
    pub endpoint: String,
    pub method: String,
    pub status_code: u16,
    pub response_time_ms: u64,
    pub sample_lei: String,
    pub response_file: Option<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EdgeCaseResult {
    pub case_type: String,
    pub lei: String,
    pub description: String,
    pub observed_behavior: String,
    pub response_file: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct RateLimitStats {
    pub total_requests: usize,
    pub total_time_ms: u64,
    pub avg_delay_ms: u64,
    pub rate_limit_hits: usize,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct TestSummary {
    pub total_endpoints: usize,
    pub successful_endpoints: usize,
    pub failed_endpoints: usize,
    pub unique_relationship_types: usize,
    pub unique_entity_categories: usize,
    pub edge_cases_tested: usize,
}

// =============================================================================
// Crawl Statistics
// =============================================================================

#[derive(Debug, Default, Clone, Serialize)]
pub struct CrawlStats {
    pub entities_discovered: usize,
    pub entities_imported: usize,
    pub relationships_created: usize,
    pub cbus_created: usize,
    pub funds_found: usize,
    pub corporates_found: usize,
    pub max_depth_reached: usize,
    pub api_requests: usize,
    pub errors: Vec<String>,
    pub by_category: HashMap<String, usize>,
    pub by_jurisdiction: HashMap<String, usize>,
}

// =============================================================================
// GLEIF Entity for Crawling
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GleifEntity {
    pub lei: String,
    pub name: String,
    pub category: Option<String>,
    pub status: Option<String>,
    pub jurisdiction: Option<String>,
    pub legal_form_id: Option<String>,
    pub registered_as: Option<String>,
    pub registered_at: Option<String>,
    pub corroboration_level: Option<String>,
    pub managing_lou: Option<String>,
    pub legal_address_city: Option<String>,
    pub legal_address_country: Option<String>,
    pub hq_address_city: Option<String>,
    pub hq_address_country: Option<String>,
    pub parent_lei: Option<String>,
    pub ultimate_parent_lei: Option<String>,
    pub depth: usize,
}

impl GleifEntity {
    fn from_json(json: &Value, depth: usize) -> Option<Self> {
        let attrs = json.get("attributes")?;
        let entity = attrs.get("entity")?;
        let registration = attrs.get("registration")?;

        let lei = json
            .get("id")
            .or_else(|| attrs.get("lei"))
            .and_then(|l| l.as_str())
            .map(|s| s.to_string())?;

        let name = entity
            .pointer("/legalName/name")
            .and_then(|n| n.as_str())
            .map(|s| s.to_string())?;

        Some(GleifEntity {
            lei,
            name,
            category: entity
                .get("category")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string()),
            status: entity
                .get("status")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string()),
            jurisdiction: entity
                .get("jurisdiction")
                .and_then(|j| j.as_str())
                .map(|s| s.to_string()),
            legal_form_id: entity
                .pointer("/legalForm/id")
                .and_then(|l| l.as_str())
                .map(|s| s.to_string()),
            registered_as: entity
                .get("registeredAs")
                .and_then(|r| r.as_str())
                .map(|s| s.to_string()),
            registered_at: entity
                .pointer("/registeredAt/id")
                .and_then(|r| r.as_str())
                .map(|s| s.to_string()),
            corroboration_level: registration
                .get("corroborationLevel")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string()),
            managing_lou: registration
                .get("managingLou")
                .and_then(|m| m.as_str())
                .map(|s| s.to_string()),
            legal_address_city: entity
                .pointer("/legalAddress/city")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string()),
            legal_address_country: entity
                .pointer("/legalAddress/country")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string()),
            hq_address_city: entity
                .pointer("/headquartersAddress/city")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string()),
            hq_address_country: entity
                .pointer("/headquartersAddress/country")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string()),
            parent_lei: None,
            ultimate_parent_lei: None,
            depth,
        })
    }
}

// =============================================================================
// GLEIF API Test Client
// =============================================================================

pub struct GleifTestClient {
    client: Client,
    last_request: Mutex<Instant>,
    request_count: AtomicUsize,
    config: TestConfig,
}

impl GleifTestClient {
    pub fn new(config: TestConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        // Create output directory
        if config.save_responses {
            fs::create_dir_all(&config.output_dir)?;
        }

        Ok(Self {
            client,
            last_request: Mutex::new(Instant::now()),
            request_count: AtomicUsize::new(0),
            config,
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
        self.request_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Make a raw GET request and return the response with metadata
    pub async fn get_raw(&self, url: &str) -> Result<(u16, String, u64)> {
        self.rate_limit().await;
        let start = Instant::now();

        let response = self.client.get(url).send().await?;
        let status = response.status().as_u16();
        let body = response.text().await?;
        let elapsed = start.elapsed().as_millis() as u64;

        Ok((status, body, elapsed))
    }

    /// Save response to file for documentation
    fn save_response(&self, filename: &str, content: &str) -> Option<String> {
        if !self.config.save_responses {
            return None;
        }

        let path = self.config.output_dir.join(filename);

        // Pretty-print JSON if possible
        let formatted = if let Ok(json) = serde_json::from_str::<Value>(content) {
            serde_json::to_string_pretty(&json).unwrap_or_else(|_| content.to_string())
        } else {
            content.to_string()
        };

        match fs::write(&path, &formatted) {
            Ok(_) => Some(path.to_string_lossy().to_string()),
            Err(e) => {
                eprintln!("  Warning: Failed to save {}: {}", filename, e);
                None
            }
        }
    }

    #[allow(dead_code)]
    fn log(&self, msg: &str) {
        if self.config.verbose {
            println!("  {}", msg);
        }
    }
}

// =============================================================================
// GLEIF Crawler
// =============================================================================

pub struct GleifCrawler {
    client: Client,
    config: CrawlConfig,
    visited: HashSet<String>,
    entities: HashMap<String, GleifEntity>,
    relationships: Vec<(String, String, String)>, // (from_lei, to_lei, type)
    queue: VecDeque<(String, usize)>,             // (lei, depth)
    stats: CrawlStats,
    pool: Option<PgPool>,
}

impl GleifCrawler {
    pub async fn new(config: CrawlConfig, pool: Option<PgPool>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        fs::create_dir_all(&config.output_dir)?;

        Ok(Self {
            client,
            config,
            visited: HashSet::new(),
            entities: HashMap::new(),
            relationships: Vec::new(),
            queue: VecDeque::new(),
            stats: CrawlStats::default(),
            pool,
        })
    }

    /// Rate-limited API request (non-recursive to avoid async boxing)
    async fn get(&mut self, url: &str) -> Result<Option<Value>> {
        let mut retries = 0;
        const MAX_RETRIES: usize = 3;

        loop {
            self.stats.api_requests += 1;
            sleep(Duration::from_millis(RATE_LIMIT_DELAY_MS)).await;

            let response = self
                .client
                .get(url)
                .header("Accept", "application/vnd.api+json")
                .send()
                .await?;

            let status = response.status();

            if status == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }

            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                retries += 1;
                if retries > MAX_RETRIES {
                    return Err(anyhow!(
                        "Rate limited after {} retries for {}",
                        MAX_RETRIES,
                        url
                    ));
                }
                // Back off and retry
                println!(
                    "  [RATE LIMITED] Waiting 5s before retry {} of {}...",
                    retries, MAX_RETRIES
                );
                sleep(Duration::from_secs(5)).await;
                continue;
            }

            if !status.is_success() {
                return Err(anyhow!("API error: {} for {}", status, url));
            }

            let json: Value = response.json().await?;
            return Ok(Some(json));
        }
    }

    /// Fetch a single LEI record
    async fn fetch_lei(&mut self, lei: &str, depth: usize) -> Result<Option<GleifEntity>> {
        let url = format!("{}/lei-records/{}", GLEIF_API_BASE, lei);

        if let Some(json) = self.get(&url).await? {
            if let Some(data) = json.get("data") {
                return Ok(GleifEntity::from_json(data, depth));
            }
        }

        Ok(None)
    }

    /// Fetch direct children of an entity
    async fn fetch_children(&mut self, lei: &str) -> Result<Vec<String>> {
        let mut children = Vec::new();
        let mut page = 1;

        loop {
            let url = format!(
                "{}/lei-records/{}/direct-children?page[number]={}&page[size]={}",
                GLEIF_API_BASE, lei, page, PAGE_SIZE
            );

            if let Some(json) = self.get(&url).await? {
                if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                    if data.is_empty() {
                        break;
                    }

                    for record in data {
                        if let Some(child_lei) = record.get("id").and_then(|i| i.as_str()) {
                            children.push(child_lei.to_string());
                        }
                    }

                    // Check pagination
                    let last_page = json
                        .pointer("/meta/pagination/lastPage")
                        .and_then(|p| p.as_u64())
                        .unwrap_or(1) as usize;

                    if page >= last_page {
                        break;
                    }
                    page += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(children)
    }

    /// Fetch managed funds for a fund manager
    async fn fetch_managed_funds(&mut self, lei: &str) -> Result<Vec<String>> {
        let mut funds = Vec::new();
        let mut page = 1;

        loop {
            let url = format!(
                "{}/lei-records/{}/managed-funds?page[number]={}&page[size]={}",
                GLEIF_API_BASE, lei, page, PAGE_SIZE
            );

            if let Some(json) = self.get(&url).await? {
                if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                    if data.is_empty() {
                        break;
                    }

                    for record in data {
                        if let Some(fund_lei) = record.get("id").and_then(|i| i.as_str()) {
                            funds.push(fund_lei.to_string());
                        }
                    }

                    let last_page = json
                        .pointer("/meta/pagination/lastPage")
                        .and_then(|p| p.as_u64())
                        .unwrap_or(1) as usize;

                    if page >= last_page {
                        break;
                    }
                    page += 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(funds)
    }

    /// Fetch direct parent relationship
    async fn fetch_parent(&mut self, lei: &str) -> Result<Option<String>> {
        let url = format!(
            "{}/lei-records/{}/direct-parent-relationship",
            GLEIF_API_BASE, lei
        );

        if let Some(json) = self.get(&url).await? {
            // Parent is in endNode (the child IS_DIRECTLY_CONSOLIDATED_BY the parent)
            if let Some(parent_lei) = json
                .pointer("/data/attributes/relationship/endNode/id")
                .and_then(|n| n.as_str())
            {
                return Ok(Some(parent_lei.to_string()));
            }
        }

        Ok(None)
    }

    /// Run the full crawl
    pub async fn crawl(&mut self) -> Result<CrawlStats> {
        println!("\n{}", "=".repeat(70));
        println!("  GLEIF FULL CRAWL");
        println!("  Starting from: {}", self.config.root_lei);
        println!(
            "  Max depth: {}, Max entities: {}",
            self.config.max_depth, self.config.max_entities
        );
        println!("{}\n", "=".repeat(70));

        let start_time = Instant::now();

        // Start with root entity
        self.queue.push_back((self.config.root_lei.clone(), 0));

        while let Some((lei, depth)) = self.queue.pop_front() {
            // Check limits
            if self.entities.len() >= self.config.max_entities {
                println!(
                    "\n  [LIMIT] Reached max entities limit: {}",
                    self.config.max_entities
                );
                break;
            }

            if depth > self.config.max_depth {
                continue;
            }

            // Skip if already visited
            if self.visited.contains(&lei) {
                continue;
            }
            self.visited.insert(lei.clone());

            // Track max depth
            if depth > self.stats.max_depth_reached {
                self.stats.max_depth_reached = depth;
            }

            // Fetch entity
            if self.config.verbose {
                println!("  [FETCH] {} (depth {})", lei, depth);
            }

            match self.fetch_lei(&lei, depth).await {
                Ok(Some(mut entity)) => {
                    self.stats.entities_discovered += 1;

                    // Track by category
                    let category = entity
                        .category
                        .clone()
                        .unwrap_or_else(|| "UNKNOWN".to_string());
                    *self.stats.by_category.entry(category.clone()).or_insert(0) += 1;

                    if category == "FUND" {
                        self.stats.funds_found += 1;
                    } else {
                        self.stats.corporates_found += 1;
                    }

                    // Track by jurisdiction
                    if let Some(ref jur) = entity.jurisdiction {
                        *self.stats.by_jurisdiction.entry(jur.clone()).or_insert(0) += 1;
                    }

                    // Progress output
                    if self.stats.entities_discovered % 10 == 0 || self.config.verbose {
                        println!(
                            "  [{}] {} - {} ({}) depth={}",
                            self.stats.entities_discovered, lei, entity.name, category, depth
                        );
                    }

                    // Fetch and queue children
                    if self.config.crawl_children && depth < self.config.max_depth {
                        match self.fetch_children(&lei).await {
                            Ok(children) => {
                                for child_lei in children {
                                    if !self.visited.contains(&child_lei) {
                                        self.relationships.push((
                                            lei.clone(),
                                            child_lei.clone(),
                                            "OWNS".to_string(),
                                        ));
                                        self.queue.push_back((child_lei, depth + 1));
                                    }
                                }
                            }
                            Err(e) => {
                                self.stats
                                    .errors
                                    .push(format!("Children fetch for {}: {}", lei, e));
                            }
                        }
                    }

                    // Fetch and queue managed funds
                    if self.config.crawl_managed_funds && depth < self.config.max_depth {
                        match self.fetch_managed_funds(&lei).await {
                            Ok(funds) => {
                                for fund_lei in funds {
                                    if !self.visited.contains(&fund_lei) {
                                        self.relationships.push((
                                            lei.clone(),
                                            fund_lei.clone(),
                                            "MANAGES".to_string(),
                                        ));
                                        self.queue.push_back((fund_lei, depth + 1));
                                    }
                                }
                            }
                            Err(e) => {
                                self.stats
                                    .errors
                                    .push(format!("Managed funds fetch for {}: {}", lei, e));
                            }
                        }
                    }

                    // Fetch parent (walk upward)
                    if self.config.crawl_parents {
                        match self.fetch_parent(&lei).await {
                            Ok(Some(parent_lei)) => {
                                entity.parent_lei = Some(parent_lei.clone());
                                if !self.visited.contains(&parent_lei) {
                                    self.relationships.push((
                                        parent_lei.clone(),
                                        lei.clone(),
                                        "OWNS".to_string(),
                                    ));
                                    // Add parent to front of queue (prioritize upward traversal)
                                    self.queue.push_front((parent_lei, depth));
                                }
                            }
                            Ok(None) => {}
                            Err(e) => {
                                self.stats
                                    .errors
                                    .push(format!("Parent fetch for {}: {}", lei, e));
                            }
                        }
                    }

                    self.entities.insert(lei, entity);
                }
                Ok(None) => {
                    if self.config.verbose {
                        println!("  [SKIP] {} - not found", lei);
                    }
                }
                Err(e) => {
                    self.stats.errors.push(format!("Fetch {}: {}", lei, e));
                }
            }
        }

        let elapsed = start_time.elapsed();

        // Import to database if not dry run
        if !self.config.dry_run {
            if let Some(pool) = self.pool.clone() {
                println!(
                    "\n  Importing {} entities to database...",
                    self.entities.len()
                );
                self.import_to_database(&pool).await?;
            } else {
                println!("\n  [WARN] No database connection - skipping import");
            }
        }

        // Save crawl results
        self.save_results()?;

        // Print summary
        self.print_summary(elapsed);

        Ok(self.stats.clone())
    }

    /// Import crawled entities to database via DSL
    async fn import_to_database(&mut self, pool: &PgPool) -> Result<()> {
        // Get or create entity types
        let fund_type_id = self
            .get_or_create_entity_type(pool, "fund_subfund", "Fund (Sub-fund)")
            .await?;
        let company_type_id = self
            .get_or_create_entity_type(pool, "limited_company", "Limited Company")
            .await?;

        let mut lei_to_entity_id: HashMap<String, Uuid> = HashMap::new();

        // Phase 1: Import all entities
        println!("  Phase 1: Importing entities...");
        for (i, (lei, entity)) in self.entities.iter().enumerate() {
            let entity_type_id = if entity.category.as_deref() == Some("FUND") {
                fund_type_id
            } else {
                company_type_id
            };

            match self.upsert_entity(pool, entity, entity_type_id).await {
                Ok(entity_id) => {
                    lei_to_entity_id.insert(lei.clone(), entity_id);
                    self.stats.entities_imported += 1;

                    if (i + 1) % 50 == 0 {
                        println!("    Imported {}/{} entities", i + 1, self.entities.len());
                    }
                }
                Err(e) => {
                    self.stats.errors.push(format!("Import {}: {}", lei, e));
                }
            }
        }

        // Phase 2: Create relationships
        println!(
            "  Phase 2: Creating {} relationships...",
            self.relationships.len()
        );
        for (from_lei, to_lei, rel_type) in &self.relationships {
            if let (Some(&from_id), Some(&to_id)) =
                (lei_to_entity_id.get(from_lei), lei_to_entity_id.get(to_lei))
            {
                match self
                    .create_relationship(pool, from_id, to_id, rel_type)
                    .await
                {
                    Ok(created) => {
                        if created {
                            self.stats.relationships_created += 1;
                        }
                    }
                    Err(e) => {
                        self.stats
                            .errors
                            .push(format!("Relationship {} -> {}: {}", from_lei, to_lei, e));
                    }
                }
            }
        }

        // Phase 3: Create CBUs for funds
        println!("  Phase 3: Creating CBUs for funds...");
        for (lei, entity) in &self.entities {
            if entity.category.as_deref() == Some("FUND") {
                if let Some(&entity_id) = lei_to_entity_id.get(lei) {
                    match self
                        .create_cbu_for_fund(
                            pool,
                            entity_id,
                            &entity.name,
                            entity.jurisdiction.as_deref(),
                        )
                        .await
                    {
                        Ok(created) => {
                            if created {
                                self.stats.cbus_created += 1;
                            }
                        }
                        Err(e) => {
                            self.stats.errors.push(format!("CBU for {}: {}", lei, e));
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn get_or_create_entity_type(
        &self,
        pool: &PgPool,
        type_code: &str,
        name: &str,
    ) -> Result<Uuid> {
        let existing: Option<(Uuid,)> = sqlx::query_as(
            r#"SELECT entity_type_id FROM "ob-poc".entity_types WHERE type_code = $1"#,
        )
        .bind(type_code)
        .fetch_optional(pool)
        .await?;

        if let Some((id,)) = existing {
            return Ok(id);
        }

        let id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO "ob-poc".entity_types (entity_type_id, type_code, name, table_name) VALUES ($1, $2, $3, $4)"#
        )
        .bind(id)
        .bind(type_code)
        .bind(name)
        .bind(format!("entity_{}", type_code.replace('-', "_")))
        .execute(pool)
        .await?;

        Ok(id)
    }

    async fn upsert_entity(
        &self,
        pool: &PgPool,
        entity: &GleifEntity,
        entity_type_id: Uuid,
    ) -> Result<Uuid> {
        // Check if entity with this LEI already exists
        let existing: Option<(Uuid,)> =
            sqlx::query_as(r#"SELECT entity_id FROM "ob-poc".entity_funds WHERE lei = $1"#)
                .bind(&entity.lei)
                .fetch_optional(pool)
                .await?;

        if let Some((entity_id,)) = existing {
            // Update existing
            sqlx::query(
                r#"
                UPDATE "ob-poc".entity_funds SET
                    gleif_category = $2,
                    gleif_status = $3,
                    gleif_legal_form_id = $4,
                    gleif_registered_as = $5,
                    gleif_registered_at = $6,
                    gleif_corroboration_level = $7,
                    gleif_managing_lou = $8,
                    legal_address_city = $9,
                    legal_address_country = $10,
                    hq_address_city = $11,
                    hq_address_country = $12,
                    jurisdiction = COALESCE(jurisdiction, $13),
                    updated_at = now()
                WHERE lei = $1
                "#,
            )
            .bind(&entity.lei)
            .bind(&entity.category)
            .bind(&entity.status)
            .bind(&entity.legal_form_id)
            .bind(&entity.registered_as)
            .bind(&entity.registered_at)
            .bind(&entity.corroboration_level)
            .bind(&entity.managing_lou)
            .bind(&entity.legal_address_city)
            .bind(&entity.legal_address_country)
            .bind(&entity.hq_address_city)
            .bind(&entity.hq_address_country)
            .bind(&entity.jurisdiction)
            .execute(pool)
            .await?;

            // Update entity name
            sqlx::query(
                r#"UPDATE "ob-poc".entities SET name = $2, updated_at = now() WHERE entity_id = $1"#
            )
            .bind(entity_id)
            .bind(&entity.name)
            .execute(pool)
            .await?;

            return Ok(entity_id);
        }

        // Create new entity
        let entity_id = Uuid::new_v4();
        let mut tx = pool.begin().await?;

        sqlx::query(
            r#"INSERT INTO "ob-poc".entities (entity_id, entity_type_id, name) VALUES ($1, $2, $3)"#
        )
        .bind(entity_id)
        .bind(entity_type_id)
        .bind(&entity.name)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_funds (
                entity_id, lei, jurisdiction,
                gleif_category, gleif_status, gleif_legal_form_id,
                gleif_registered_as, gleif_registered_at,
                gleif_corroboration_level, gleif_managing_lou,
                legal_address_city, legal_address_country,
                hq_address_city, hq_address_country
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
        )
        .bind(entity_id)
        .bind(&entity.lei)
        .bind(&entity.jurisdiction)
        .bind(&entity.category)
        .bind(&entity.status)
        .bind(&entity.legal_form_id)
        .bind(&entity.registered_as)
        .bind(&entity.registered_at)
        .bind(&entity.corroboration_level)
        .bind(&entity.managing_lou)
        .bind(&entity.legal_address_city)
        .bind(&entity.legal_address_country)
        .bind(&entity.hq_address_city)
        .bind(&entity.hq_address_country)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(entity_id)
    }

    async fn create_relationship(
        &self,
        pool: &PgPool,
        from_id: Uuid,
        to_id: Uuid,
        rel_type: &str,
    ) -> Result<bool> {
        // Check if exists
        let exists: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".entity_relationships
                WHERE from_entity_id = $1 AND to_entity_id = $2
            )
            "#,
        )
        .bind(from_id)
        .bind(to_id)
        .fetch_one(pool)
        .await?;

        if exists {
            return Ok(false);
        }

        let relationship_type = match rel_type {
            "OWNS" => "ownership",
            "MANAGES" => "control",
            _ => "ownership",
        };

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".entity_relationships (
                relationship_id, from_entity_id, to_entity_id, relationship_type,
                percentage, source, effective_from
            ) VALUES ($1, $2, $3, $4, 100.00, 'GLEIF', CURRENT_DATE)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(from_id)
        .bind(to_id)
        .bind(relationship_type)
        .execute(pool)
        .await?;

        Ok(true)
    }

    async fn create_cbu_for_fund(
        &self,
        pool: &PgPool,
        entity_id: Uuid,
        name: &str,
        jurisdiction: Option<&str>,
    ) -> Result<bool> {
        // Check if CBU exists
        let exists: bool =
            sqlx::query_scalar(r#"SELECT EXISTS(SELECT 1 FROM "ob-poc".cbus WHERE name = $1)"#)
                .bind(name)
                .fetch_one(pool)
                .await?;

        if exists {
            return Ok(false);
        }

        // Create CBU
        let cbu_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbus (cbu_id, name, jurisdiction, client_type)
            VALUES ($1, $2, $3, 'fund')
            "#,
        )
        .bind(cbu_id)
        .bind(name)
        .bind(jurisdiction)
        .execute(pool)
        .await?;

        // Get or create ASSET_OWNER role
        let role_id = self.get_or_create_role(pool, "ASSET_OWNER").await?;

        // Assign role
        sqlx::query(
            r#"
            INSERT INTO "ob-poc".cbu_entity_roles (cbu_entity_role_id, cbu_id, entity_id, role_id)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(cbu_id)
        .bind(entity_id)
        .bind(role_id)
        .execute(pool)
        .await?;

        Ok(true)
    }

    async fn get_or_create_role(&self, pool: &PgPool, role_name: &str) -> Result<Uuid> {
        let existing: Option<(Uuid,)> =
            sqlx::query_as(r#"SELECT role_id FROM "ob-poc".roles WHERE name = $1"#)
                .bind(role_name)
                .fetch_optional(pool)
                .await?;

        if let Some((id,)) = existing {
            return Ok(id);
        }

        let id = Uuid::new_v4();
        sqlx::query(r#"INSERT INTO "ob-poc".roles (role_id, name) VALUES ($1, $2)"#)
            .bind(id)
            .bind(role_name)
            .execute(pool)
            .await?;
        Ok(id)
    }

    fn save_results(&self) -> Result<()> {
        // Save entities
        let entities_path = self.config.output_dir.join("crawl_entities.json");
        let entities_json = serde_json::to_string_pretty(&self.entities)?;
        fs::write(&entities_path, &entities_json)?;

        // Save relationships
        let rels_path = self.config.output_dir.join("crawl_relationships.json");
        let rels_json = serde_json::to_string_pretty(&self.relationships)?;
        fs::write(&rels_path, &rels_json)?;

        // Save stats
        let stats_path = self.config.output_dir.join("crawl_stats.json");
        let stats_json = serde_json::to_string_pretty(&self.stats)?;
        fs::write(&stats_path, &stats_json)?;

        println!("\n  Results saved to:");
        println!("    - {}", entities_path.display());
        println!("    - {}", rels_path.display());
        println!("    - {}", stats_path.display());

        Ok(())
    }

    fn print_summary(&self, elapsed: Duration) {
        println!("\n{}", "=".repeat(70));
        println!("  CRAWL SUMMARY");
        println!("{}", "=".repeat(70));

        println!("\n  ENTITIES:");
        println!("    Discovered: {}", self.stats.entities_discovered);
        println!("    Imported:   {}", self.stats.entities_imported);
        println!("    Funds:      {}", self.stats.funds_found);
        println!("    Corporates: {}", self.stats.corporates_found);

        println!("\n  RELATIONSHIPS:");
        println!("    Found:   {}", self.relationships.len());
        println!("    Created: {}", self.stats.relationships_created);

        println!("\n  CBUS:");
        println!("    Created: {}", self.stats.cbus_created);

        println!("\n  BY CATEGORY:");
        for (cat, count) in &self.stats.by_category {
            println!("    {}: {}", cat, count);
        }

        println!("\n  TOP JURISDICTIONS:");
        let mut jur_vec: Vec<_> = self.stats.by_jurisdiction.iter().collect();
        jur_vec.sort_by(|a, b| b.1.cmp(a.1));
        for (jur, count) in jur_vec.iter().take(10) {
            println!("    {}: {}", jur, count);
        }

        println!("\n  PERFORMANCE:");
        println!("    Max depth:    {}", self.stats.max_depth_reached);
        println!("    API requests: {}", self.stats.api_requests);
        println!("    Total time:   {:.1}s", elapsed.as_secs_f64());
        println!(
            "    Avg req time: {:.0}ms",
            if self.stats.api_requests > 0 {
                elapsed.as_millis() as f64 / self.stats.api_requests as f64
            } else {
                0.0
            }
        );

        if !self.stats.errors.is_empty() {
            println!("\n  ERRORS ({}):", self.stats.errors.len());
            for (i, err) in self.stats.errors.iter().take(10).enumerate() {
                println!("    {}. {}", i + 1, err);
            }
            if self.stats.errors.len() > 10 {
                println!("    ... and {} more", self.stats.errors.len() - 10);
            }
        }

        println!("\n{}", "=".repeat(70));
    }
}

// =============================================================================
// Test Harness (Original)
// =============================================================================

pub struct GleifTestHarness {
    client: GleifTestClient,
    report: TestReport,
    start_time: Instant,
}

impl GleifTestHarness {
    pub async fn new(config: TestConfig) -> Result<Self> {
        Ok(Self {
            client: GleifTestClient::new(config)?,
            report: TestReport::default(),
            start_time: Instant::now(),
        })
    }

    /// Run all tests and generate report
    pub async fn run_all_tests(&mut self) -> Result<()> {
        println!("\n{}", "=".repeat(70));
        println!("  GLEIF API TEST HARNESS");
        println!("  Testing API endpoints, response structures, and edge cases");
        println!("{}\n", "=".repeat(70));

        // 1. Core endpoint tests
        self.test_lei_record_endpoint().await;
        self.test_search_endpoint().await;
        self.test_managed_funds_endpoint().await;

        // 2. Relationship endpoints
        self.test_direct_parent_endpoint().await;
        self.test_ultimate_parent_endpoint().await;
        self.test_direct_children_endpoint().await;

        // 3. Fund structure relationships
        self.test_umbrella_fund_relationship().await;
        self.test_fund_manager_relationship().await;
        self.test_master_feeder_relationship().await;

        // 4. Edge cases
        self.test_sicav_structure().await;
        self.test_inactive_merged_lei().await;
        self.test_missing_parent().await;
        self.test_reporting_exception().await;

        // 5. Finalize report
        self.finalize_report();
        self.print_report();
        self.save_report()?;

        Ok(())
    }

    // =========================================================================
    // Core Endpoint Tests
    // =========================================================================

    async fn test_lei_record_endpoint(&mut self) {
        println!("\n{}", "-".repeat(50));
        println!("  Endpoint: GET /lei-records/{{lei}}");
        println!("{}", "-".repeat(50));

        let url = format!("{}/lei-records/{}", GLEIF_API_BASE, ALLIANZ_SE_LEI);

        match self.client.get_raw(&url).await {
            Ok((status, body, time_ms)) => {
                let file = self.client.save_response("lei_record_example.json", &body);

                // Extract metadata from response
                if let Ok(json) = serde_json::from_str::<Value>(&body) {
                    self.extract_entity_metadata(&json);
                }

                let result = EndpointResult {
                    endpoint: "/lei-records/{lei}".to_string(),
                    method: "GET".to_string(),
                    status_code: status,
                    response_time_ms: time_ms,
                    sample_lei: ALLIANZ_SE_LEI.to_string(),
                    response_file: file,
                    notes: vec![
                        "Returns full LEI record with entity details".to_string(),
                        "Includes relationships links".to_string(),
                    ],
                };

                let icon = if status == 200 { "[OK]" } else { "[FAIL]" };
                println!("  {} Status: {} ({}ms)", icon, status, time_ms);

                self.report.endpoints_tested.push(result);
            }
            Err(e) => {
                self.report
                    .errors
                    .push(format!("lei-records endpoint: {}", e));
                println!("  [FAIL] Error: {}", e);
            }
        }
    }

    async fn test_search_endpoint(&mut self) {
        println!("\n{}", "-".repeat(50));
        println!("  Endpoint: GET /lei-records?filter[entity.legalName]=...");
        println!("{}", "-".repeat(50));

        let search_term = "Allianz Global Investors";
        let url = format!(
            "{}/lei-records?filter[entity.legalName]={}&page[size]=5",
            GLEIF_API_BASE,
            search_term.replace(' ', "%20")
        );

        match self.client.get_raw(&url).await {
            Ok((status, body, time_ms)) => {
                let file = self
                    .client
                    .save_response("search_results_example.json", &body);

                let mut notes = vec![];
                if let Ok(json) = serde_json::from_str::<Value>(&body) {
                    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                        notes.push(format!("Returned {} results", data.len()));
                        for record in data {
                            self.extract_entity_metadata_from_record(record);
                        }
                    }
                    if let Some(meta) = json.get("meta") {
                        if let Some(pagination) = meta.get("pagination") {
                            notes.push(format!("Pagination: {:?}", pagination));
                        }
                    }
                }

                let result = EndpointResult {
                    endpoint: "/lei-records?filter[entity.legalName]=...".to_string(),
                    method: "GET".to_string(),
                    status_code: status,
                    response_time_ms: time_ms,
                    sample_lei: search_term.to_string(),
                    response_file: file,
                    notes,
                };

                let icon = if status == 200 { "[OK]" } else { "[FAIL]" };
                println!("  {} Status: {} ({}ms)", icon, status, time_ms);

                self.report.endpoints_tested.push(result);
            }
            Err(e) => {
                self.report.errors.push(format!("search endpoint: {}", e));
                println!("  [FAIL] Error: {}", e);
            }
        }
    }

    async fn test_managed_funds_endpoint(&mut self) {
        println!("\n{}", "-".repeat(50));
        println!("  Endpoint: GET /lei-records/{{lei}}/managed-funds");
        println!("{}", "-".repeat(50));

        let url = format!(
            "{}/lei-records/{}/managed-funds?page[size]={}",
            GLEIF_API_BASE, ALLIANZ_GI_LEI, self.client.config.max_managed_funds
        );

        match self.client.get_raw(&url).await {
            Ok((status, body, time_ms)) => {
                let file = self
                    .client
                    .save_response("managed_funds_example.json", &body);

                let mut notes = vec![];
                if let Ok(json) = serde_json::from_str::<Value>(&body) {
                    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                        notes.push(format!("Found {} managed funds", data.len()));

                        // Extract unique categories from funds
                        let mut fund_categories: HashSet<String> = HashSet::new();
                        for record in data {
                            if let Some(cat) = record
                                .pointer("/attributes/entity/category")
                                .and_then(|c| c.as_str())
                            {
                                fund_categories.insert(cat.to_string());
                            }
                            self.extract_entity_metadata_from_record(record);
                        }
                        notes.push(format!("Fund categories: {:?}", fund_categories));
                    }
                }

                let result = EndpointResult {
                    endpoint: "/lei-records/{lei}/managed-funds".to_string(),
                    method: "GET".to_string(),
                    status_code: status,
                    response_time_ms: time_ms,
                    sample_lei: ALLIANZ_GI_LEI.to_string(),
                    response_file: file,
                    notes,
                };

                let icon = if status == 200 { "[OK]" } else { "[FAIL]" };
                println!("  {} Status: {} ({}ms)", icon, status, time_ms);

                self.report.endpoints_tested.push(result);
            }
            Err(e) => {
                self.report
                    .errors
                    .push(format!("managed-funds endpoint: {}", e));
                println!("  [FAIL] Error: {}", e);
            }
        }
    }

    // =========================================================================
    // Relationship Endpoint Tests
    // =========================================================================

    async fn test_direct_parent_endpoint(&mut self) {
        println!("\n{}", "-".repeat(50));
        println!("  Endpoint: GET /lei-records/{{lei}}/direct-parent-relationship");
        println!("{}", "-".repeat(50));

        // First, find a child of Allianz SE that has a parent relationship
        // Get direct children first, then test parent relationship on one of them
        let children_url = format!(
            "{}/lei-records/{}/direct-children?page[size]=3",
            GLEIF_API_BASE, ALLIANZ_SE_LEI
        );

        let mut test_lei = String::new();
        if let Ok((_, body, _)) = self.client.get_raw(&children_url).await {
            if let Ok(json) = serde_json::from_str::<Value>(&body) {
                if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                    if let Some(first) = data.first() {
                        if let Some(lei) = first.get("id").and_then(|i| i.as_str()) {
                            test_lei = lei.to_string();
                            println!("  Using child entity: {}", lei);
                        }
                    }
                }
            }
        }

        if test_lei.is_empty() {
            println!("  [SKIP] Could not find child entity to test parent relationship");
            return;
        }

        let url = format!(
            "{}/lei-records/{}/direct-parent-relationship",
            GLEIF_API_BASE, test_lei
        );

        match self.client.get_raw(&url).await {
            Ok((status, body, time_ms)) => {
                let file = self
                    .client
                    .save_response("direct_parent_example.json", &body);

                let mut notes = vec![];
                if let Ok(json) = serde_json::from_str::<Value>(&body) {
                    if let Some(rel_type) = json
                        .pointer("/data/attributes/relationship/relationshipType")
                        .and_then(|t| t.as_str())
                    {
                        self.report.relationship_types.insert(rel_type.to_string());
                        notes.push(format!("Relationship type: {}", rel_type));
                    }
                    if let Some(parent_lei) = json
                        .pointer("/data/attributes/relationship/endNode/nodeId")
                        .and_then(|n| n.as_str())
                    {
                        notes.push(format!("Parent LEI: {}", parent_lei));
                    }
                }

                let result = EndpointResult {
                    endpoint: "/lei-records/{lei}/direct-parent-relationship".to_string(),
                    method: "GET".to_string(),
                    status_code: status,
                    response_time_ms: time_ms,
                    sample_lei: test_lei,
                    response_file: file,
                    notes,
                };

                let icon = if status == 200 { "[OK]" } else { "[FAIL]" };
                println!("  {} Status: {} ({}ms)", icon, status, time_ms);

                self.report.endpoints_tested.push(result);
            }
            Err(e) => {
                self.report
                    .errors
                    .push(format!("direct-parent endpoint: {}", e));
                println!("  [FAIL] Error: {}", e);
            }
        }
    }

    async fn test_ultimate_parent_endpoint(&mut self) {
        println!("\n{}", "-".repeat(50));
        println!("  Endpoint: GET /lei-records/{{lei}}/ultimate-parent-relationship");
        println!("{}", "-".repeat(50));

        // Find a child entity first (from direct-children of Allianz SE)
        let children_url = format!(
            "{}/lei-records/{}/direct-children?page[size]=3",
            GLEIF_API_BASE, ALLIANZ_SE_LEI
        );

        let mut test_lei = String::new();
        if let Ok((_, body, _)) = self.client.get_raw(&children_url).await {
            if let Ok(json) = serde_json::from_str::<Value>(&body) {
                if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                    if let Some(first) = data.first() {
                        if let Some(lei) = first.get("id").and_then(|i| i.as_str()) {
                            test_lei = lei.to_string();
                            println!("  Using child entity: {}", lei);
                        }
                    }
                }
            }
        }

        if test_lei.is_empty() {
            println!("  [SKIP] Could not find child entity to test ultimate parent");
            return;
        }

        let url = format!(
            "{}/lei-records/{}/ultimate-parent-relationship",
            GLEIF_API_BASE, test_lei
        );

        match self.client.get_raw(&url).await {
            Ok((status, body, time_ms)) => {
                let file = self
                    .client
                    .save_response("ultimate_parent_example.json", &body);

                let mut notes = vec![];
                if let Ok(json) = serde_json::from_str::<Value>(&body) {
                    if let Some(rel_type) = json
                        .pointer("/data/attributes/relationship/relationshipType")
                        .and_then(|t| t.as_str())
                    {
                        self.report.relationship_types.insert(rel_type.to_string());
                        notes.push(format!("Relationship type: {}", rel_type));
                    }
                    if let Some(parent_lei) = json
                        .pointer("/data/attributes/relationship/endNode/nodeId")
                        .and_then(|n| n.as_str())
                    {
                        notes.push(format!("Ultimate parent LEI: {}", parent_lei));
                    }
                }

                let result = EndpointResult {
                    endpoint: "/lei-records/{lei}/ultimate-parent-relationship".to_string(),
                    method: "GET".to_string(),
                    status_code: status,
                    response_time_ms: time_ms,
                    sample_lei: test_lei,
                    response_file: file,
                    notes,
                };

                let icon = if status == 200 { "[OK]" } else { "[FAIL]" };
                println!("  {} Status: {} ({}ms)", icon, status, time_ms);

                self.report.endpoints_tested.push(result);
            }
            Err(e) => {
                self.report
                    .errors
                    .push(format!("ultimate-parent endpoint: {}", e));
                println!("  [FAIL] Error: {}", e);
            }
        }
    }

    async fn test_direct_children_endpoint(&mut self) {
        println!("\n{}", "-".repeat(50));
        println!("  Endpoint: GET /lei-records/{{lei}}/direct-children");
        println!("{}", "-".repeat(50));

        // Use the correct endpoint: /lei-records/{lei}/direct-children
        let url = format!(
            "{}/lei-records/{}/direct-children?page[size]={}",
            GLEIF_API_BASE, ALLIANZ_SE_LEI, self.client.config.max_children
        );

        match self.client.get_raw(&url).await {
            Ok((status, body, time_ms)) => {
                let file = self
                    .client
                    .save_response("direct_children_example.json", &body);

                let mut notes = vec![];
                if let Ok(json) = serde_json::from_str::<Value>(&body) {
                    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                        notes.push(format!("Found {} direct children", data.len()));
                        for record in data {
                            self.extract_entity_metadata_from_record(record);
                        }
                    }
                }

                let result = EndpointResult {
                    endpoint: "/lei-records/{lei}/direct-children".to_string(),
                    method: "GET".to_string(),
                    status_code: status,
                    response_time_ms: time_ms,
                    sample_lei: ALLIANZ_SE_LEI.to_string(),
                    response_file: file,
                    notes,
                };

                let icon = if status == 200 { "[OK]" } else { "[FAIL]" };
                println!("  {} Status: {} ({}ms)", icon, status, time_ms);

                self.report.endpoints_tested.push(result);
            }
            Err(e) => {
                self.report
                    .errors
                    .push(format!("direct-children endpoint: {}", e));
                println!("  [FAIL] Error: {}", e);
            }
        }
    }

    // =========================================================================
    // Fund Structure Relationship Tests
    // =========================================================================

    async fn test_umbrella_fund_relationship(&mut self) {
        println!("\n{}", "-".repeat(50));
        println!("  Relationship: IS_SUBFUND_OF (umbrella fund)");
        println!("{}", "-".repeat(50));

        // First find a sub-fund from the managed funds
        let search_url = format!(
            "{}/lei-records/{}/managed-funds?page[size]=20",
            GLEIF_API_BASE, ALLIANZ_GI_LEI
        );

        if let Ok((_, body, _)) = self.client.get_raw(&search_url).await {
            if let Ok(json) = serde_json::from_str::<Value>(&body) {
                if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                    // Look for a fund with umbrella relationship
                    for record in data {
                        if let Some(umbrella_url) = record
                            .pointer("/relationships/umbrella-fund/links/related")
                            .and_then(|u| u.as_str())
                        {
                            let lei = record.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                            println!("  Found sub-fund with umbrella: {}", lei);

                            // Fetch the umbrella
                            if let Some(umbrella_lei) = umbrella_url.split('/').last() {
                                let umbrella_record_url =
                                    format!("{}/lei-records/{}", GLEIF_API_BASE, umbrella_lei);

                                if let Ok((status, umbrella_body, time_ms)) =
                                    self.client.get_raw(&umbrella_record_url).await
                                {
                                    let file = self.client.save_response(
                                        "umbrella_fund_example.json",
                                        &umbrella_body,
                                    );

                                    self.report
                                        .relationship_types
                                        .insert("IS_SUBFUND_OF".to_string());

                                    let mut notes = vec![
                                        format!("Sub-fund LEI: {}", lei),
                                        format!("Umbrella LEI: {}", umbrella_lei),
                                    ];

                                    if let Ok(uj) = serde_json::from_str::<Value>(&umbrella_body) {
                                        if let Some(name) = uj
                                            .pointer("/data/attributes/entity/legalName/name")
                                            .and_then(|n| n.as_str())
                                        {
                                            notes.push(format!("Umbrella name: {}", name));
                                        }
                                    }

                                    let result = EndpointResult {
                                        endpoint: "IS_SUBFUND_OF relationship".to_string(),
                                        method: "GET".to_string(),
                                        status_code: status,
                                        response_time_ms: time_ms,
                                        sample_lei: umbrella_lei.to_string(),
                                        response_file: file,
                                        notes,
                                    };

                                    println!(
                                        "  [OK] Found umbrella: {} ({}ms)",
                                        umbrella_lei, time_ms
                                    );

                                    self.report.endpoints_tested.push(result);
                                    return;
                                }
                            }
                        }
                    }
                    println!("  [WARN] No sub-fund with umbrella found in sample");
                }
            }
        }
    }

    async fn test_fund_manager_relationship(&mut self) {
        println!("\n{}", "-".repeat(50));
        println!("  Relationship: IS_FUND-MANAGED_BY");
        println!("{}", "-".repeat(50));

        // Get a fund and check for fund-manager relationship
        let search_url = format!(
            "{}/lei-records/{}/managed-funds?page[size]=5",
            GLEIF_API_BASE, ALLIANZ_GI_LEI
        );

        if let Ok((_, body, _)) = self.client.get_raw(&search_url).await {
            if let Ok(json) = serde_json::from_str::<Value>(&body) {
                if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                    if let Some(fund) = data.first() {
                        let fund_lei = fund.get("id").and_then(|i| i.as_str()).unwrap_or("?");

                        // Check if fund has manager relationship link
                        if let Some(manager_url) = fund
                            .pointer("/relationships/fund-manager/links/related")
                            .and_then(|m| m.as_str())
                        {
                            self.report
                                .relationship_types
                                .insert("IS_FUND-MANAGED_BY".to_string());
                            println!("  [OK] Found fund-manager relationship for {}", fund_lei);
                            println!("    Manager URL: {}", manager_url);
                        } else {
                            println!("  [WARN] No explicit fund-manager link in response");
                            println!("    (Manager is implied by managed-funds endpoint)");
                        }
                    }
                }
            }
        }
    }

    async fn test_master_feeder_relationship(&mut self) {
        println!("\n{}", "-".repeat(50));
        println!("  Relationship: IS_FEEDER_TO (master-feeder)");
        println!("{}", "-".repeat(50));

        // Search for funds that might be feeders
        let url = format!(
            "{}/lei-records?filter[entity.legalName]=feeder&filter[entity.category]=FUND&page[size]=5",
            GLEIF_API_BASE
        );

        match self.client.get_raw(&url).await {
            Ok((status, body, time_ms)) => {
                let mut notes = vec![];

                if let Ok(json) = serde_json::from_str::<Value>(&body) {
                    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                        notes.push(format!("Found {} potential feeder funds", data.len()));

                        for record in data {
                            if let Some(master_url) = record
                                .pointer("/relationships/master-fund/links/related")
                                .and_then(|m| m.as_str())
                            {
                                let lei = record.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                                self.report
                                    .relationship_types
                                    .insert("IS_FEEDER_TO".to_string());
                                notes.push(format!("Feeder {} -> Master at {}", lei, master_url));

                                self.client.save_response(
                                    "feeder_fund_example.json",
                                    &serde_json::to_string_pretty(record).unwrap_or_default(),
                                );
                            }
                        }
                    }
                }

                let icon = if status == 200 { "[OK]" } else { "[FAIL]" };
                println!("  {} Status: {} ({}ms)", icon, status, time_ms);

                for note in &notes {
                    println!("    {}", note);
                }
            }
            Err(e) => {
                println!("  [FAIL] Error: {}", e);
            }
        }
    }

    // =========================================================================
    // Edge Case Tests
    // =========================================================================

    async fn test_sicav_structure(&mut self) {
        println!("\n{}", "-".repeat(50));
        println!("  Edge Case: SICAV (self-governing fund structure)");
        println!("{}", "-".repeat(50));

        // Search for SICAVs
        let url = format!(
            "{}/lei-records?filter[entity.legalName]=SICAV&filter[entity.category]=FUND&page[size]=3",
            GLEIF_API_BASE
        );

        match self.client.get_raw(&url).await {
            Ok((status, body, _time_ms)) => {
                let file = self.client.save_response("sicav_examples.json", &body);

                if let Ok(json) = serde_json::from_str::<Value>(&body) {
                    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                        for record in data.iter().take(1) {
                            let lei = record.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                            let name = record
                                .pointer("/attributes/entity/legalName/name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("?");

                            // Check for parent relationships
                            let has_umbrella = record
                                .pointer("/relationships/umbrella-fund/links/related")
                                .is_some();
                            let has_parent = record
                                .pointer("/relationships/direct-parent/links/related")
                                .is_some();

                            let edge_case = EdgeCaseResult {
                                case_type: "SICAV".to_string(),
                                lei: lei.to_string(),
                                description: format!("SICAV fund: {}", name),
                                observed_behavior: format!(
                                    "has_umbrella: {}, has_parent: {}",
                                    has_umbrella, has_parent
                                ),
                                response_file: file.clone(),
                            };

                            println!("  [OK] SICAV: {} ({})", name, lei);
                            println!(
                                "    Has umbrella: {}, Has parent: {}",
                                has_umbrella, has_parent
                            );

                            self.report.edge_cases.push(edge_case);
                        }
                    }
                }

                let _ = status; // Use status to avoid warning
            }
            Err(e) => {
                println!("  [FAIL] Error: {}", e);
            }
        }
    }

    async fn test_inactive_merged_lei(&mut self) {
        println!("\n{}", "-".repeat(50));
        println!("  Edge Case: Inactive/Merged LEI");
        println!("{}", "-".repeat(50));

        // Search for inactive entities
        let url = format!(
            "{}/lei-records?filter[entity.status]=INACTIVE&page[size]=3",
            GLEIF_API_BASE
        );

        match self.client.get_raw(&url).await {
            Ok((status, body, _time_ms)) => {
                let file = self
                    .client
                    .save_response("inactive_lei_examples.json", &body);

                if let Ok(json) = serde_json::from_str::<Value>(&body) {
                    if let Some(data) = json.get("data").and_then(|d| d.as_array()) {
                        println!("  Found {} inactive LEIs", data.len());

                        for record in data.iter().take(2) {
                            let lei = record.get("id").and_then(|i| i.as_str()).unwrap_or("?");
                            let name = record
                                .pointer("/attributes/entity/legalName/name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("?");
                            let status_val = record
                                .pointer("/attributes/entity/status")
                                .and_then(|s| s.as_str())
                                .unwrap_or("?");

                            // Check for successor
                            let successor = record
                                .pointer("/attributes/entity/successorEntities")
                                .and_then(|s| s.as_array())
                                .map(|arr| !arr.is_empty())
                                .unwrap_or(false);

                            self.report.entity_statuses.insert(status_val.to_string());

                            let edge_case = EdgeCaseResult {
                                case_type: "INACTIVE_LEI".to_string(),
                                lei: lei.to_string(),
                                description: format!("{} ({})", name, status_val),
                                observed_behavior: format!("has_successor: {}", successor),
                                response_file: file.clone(),
                            };

                            println!("  [OK] {} - {} (successor: {})", lei, status_val, successor);

                            self.report.edge_cases.push(edge_case);
                        }
                    }
                }

                let _ = status; // Use status to avoid warning
            }
            Err(e) => {
                println!("  [FAIL] Error: {}", e);
            }
        }
    }

    async fn test_missing_parent(&mut self) {
        println!("\n{}", "-".repeat(50));
        println!("  Edge Case: Entity with no parent");
        println!("{}", "-".repeat(50));

        // Try to get parent of Allianz SE (should be at top of chain)
        let url = format!(
            "{}/lei-records/{}/direct-parent-relationship",
            GLEIF_API_BASE, ALLIANZ_SE_LEI
        );

        match self.client.get_raw(&url).await {
            Ok((status, body, _time_ms)) => {
                let file = self.client.save_response("no_parent_example.json", &body);

                let edge_case = EdgeCaseResult {
                    case_type: "NO_PARENT".to_string(),
                    lei: ALLIANZ_SE_LEI.to_string(),
                    description: "Top-level entity with no corporate parent".to_string(),
                    observed_behavior: format!(
                        "Status: {}, Response size: {} bytes",
                        status,
                        body.len()
                    ),
                    response_file: file,
                };

                if status == 404 {
                    println!("  [OK] No parent found (404) - expected for top-level");
                } else if status == 200 {
                    // Check if response indicates no parent
                    if body.contains("NO_KNOWN_PERSON") || body.contains("NON_PUBLIC") {
                        println!("  [OK] Parent is a reporting exception");
                    } else {
                        println!("  [OK] Has parent relationship");
                    }
                }

                self.report.edge_cases.push(edge_case);
            }
            Err(e) => {
                println!("  [FAIL] Error: {}", e);
            }
        }
    }

    async fn test_reporting_exception(&mut self) {
        println!("\n{}", "-".repeat(50));
        println!("  Edge Case: Reporting Exception");
        println!("{}", "-".repeat(50));

        // Get the full record to check for exceptions
        let url = format!("{}/lei-records/{}", GLEIF_API_BASE, ALLIANZ_SE_LEI);

        match self.client.get_raw(&url).await {
            Ok((_status, body, _)) => {
                if let Ok(json) = serde_json::from_str::<Value>(&body) {
                    // Check for exception reasons
                    let direct_exception = json
                        .pointer("/data/relationships/direct-parent-reporting-exception")
                        .and_then(|e| e.get("links"))
                        .is_some();

                    let ultimate_exception = json
                        .pointer("/data/relationships/ultimate-parent-reporting-exception")
                        .and_then(|e| e.get("links"))
                        .is_some();

                    if direct_exception || ultimate_exception {
                        self.client
                            .save_response("reporting_exception_example.json", &body);

                        let edge_case = EdgeCaseResult {
                            case_type: "REPORTING_EXCEPTION".to_string(),
                            lei: ALLIANZ_SE_LEI.to_string(),
                            description: "Entity with reporting exception for parent".to_string(),
                            observed_behavior: format!(
                                "direct_exception: {}, ultimate_exception: {}",
                                direct_exception, ultimate_exception
                            ),
                            response_file: Some("reporting_exception_example.json".to_string()),
                        };

                        println!("  [OK] Found reporting exception");
                        println!(
                            "    Direct: {}, Ultimate: {}",
                            direct_exception, ultimate_exception
                        );

                        self.report.edge_cases.push(edge_case);
                    } else {
                        println!("  [WARN] No reporting exception in this entity");
                    }
                }
            }
            Err(e) => {
                println!("  [FAIL] Error: {}", e);
            }
        }
    }

    // =========================================================================
    // Metadata Extraction Helpers
    // =========================================================================

    fn extract_entity_metadata(&mut self, json: &Value) {
        if let Some(data) = json.get("data") {
            self.extract_entity_metadata_from_record(data);
        }
    }

    fn extract_entity_metadata_from_record(&mut self, record: &Value) {
        // Category
        if let Some(cat) = record
            .pointer("/attributes/entity/category")
            .and_then(|c| c.as_str())
        {
            self.report.entity_categories.insert(cat.to_string());
        }

        // Status
        if let Some(status) = record
            .pointer("/attributes/entity/status")
            .and_then(|s| s.as_str())
        {
            self.report.entity_statuses.insert(status.to_string());
        }

        // Legal form
        if let Some(legal_form) = record
            .pointer("/attributes/entity/legalForm/id")
            .and_then(|l| l.as_str())
        {
            self.report.legal_forms.insert(legal_form.to_string());
        }

        // Relationship types from relationships object
        if let Some(rels) = record.get("relationships").and_then(|r| r.as_object()) {
            for (rel_type, _) in rels {
                // Convert key format to relationship type
                let type_name = rel_type.replace('-', "_").to_uppercase();
                self.report.relationship_types.insert(type_name);
            }
        }
    }

    // =========================================================================
    // Report Finalization
    // =========================================================================

    fn finalize_report(&mut self) {
        let elapsed = self.start_time.elapsed();
        let request_count = self.client.request_count.load(Ordering::SeqCst);

        self.report.rate_limit_stats = RateLimitStats {
            total_requests: request_count,
            total_time_ms: elapsed.as_millis() as u64,
            avg_delay_ms: if request_count > 0 {
                elapsed.as_millis() as u64 / request_count as u64
            } else {
                0
            },
            rate_limit_hits: 0, // We don't track this yet
        };

        self.report.summary = TestSummary {
            total_endpoints: self.report.endpoints_tested.len(),
            successful_endpoints: self
                .report
                .endpoints_tested
                .iter()
                .filter(|e| e.status_code == 200)
                .count(),
            failed_endpoints: self
                .report
                .endpoints_tested
                .iter()
                .filter(|e| e.status_code != 200)
                .count(),
            unique_relationship_types: self.report.relationship_types.len(),
            unique_entity_categories: self.report.entity_categories.len(),
            edge_cases_tested: self.report.edge_cases.len(),
        };
    }

    fn print_report(&self) {
        println!("\n{}", "=".repeat(70));
        println!("  TEST REPORT");
        println!("{}", "=".repeat(70));

        println!("\n  ENDPOINT COVERAGE:");
        for ep in &self.report.endpoints_tested {
            let status_icon = if ep.status_code == 200 {
                "[OK]"
            } else {
                "[FAIL]"
            };
            println!(
                "    {} {} {} ({}ms)",
                status_icon, ep.method, ep.endpoint, ep.response_time_ms
            );
        }

        println!("\n  RELATIONSHIP TYPES DISCOVERED:");
        for rel in &self.report.relationship_types {
            println!("    - {}", rel);
        }

        println!("\n  ENTITY CATEGORIES:");
        for cat in &self.report.entity_categories {
            println!("    - {}", cat);
        }

        println!("\n  ENTITY STATUSES:");
        for status in &self.report.entity_statuses {
            println!("    - {}", status);
        }

        println!("\n  LEGAL FORMS:");
        for form in self.report.legal_forms.iter().take(10) {
            println!("    - {}", form);
        }
        if self.report.legal_forms.len() > 10 {
            println!("    ... and {} more", self.report.legal_forms.len() - 10);
        }

        println!("\n  EDGE CASES:");
        for ec in &self.report.edge_cases {
            println!(
                "    {} [{}]: {}",
                ec.case_type, ec.lei, ec.observed_behavior
            );
        }

        println!("\n  RATE LIMITING:");
        println!(
            "    Total requests: {}",
            self.report.rate_limit_stats.total_requests
        );
        println!(
            "    Total time: {}ms",
            self.report.rate_limit_stats.total_time_ms
        );
        println!(
            "    Avg per request: {}ms",
            self.report.rate_limit_stats.avg_delay_ms
        );

        if !self.report.errors.is_empty() {
            println!("\n  ERRORS:");
            for err in &self.report.errors {
                println!("    [FAIL] {}", err);
            }
        }

        println!("\n  SUMMARY:");
        println!(
            "    Endpoints tested: {}/{} successful",
            self.report.summary.successful_endpoints, self.report.summary.total_endpoints
        );
        println!(
            "    Relationship types: {}",
            self.report.summary.unique_relationship_types
        );
        println!(
            "    Entity categories: {}",
            self.report.summary.unique_entity_categories
        );
        println!("    Edge cases: {}", self.report.summary.edge_cases_tested);

        println!("\n{}\n", "=".repeat(70));
    }

    fn save_report(&self) -> Result<()> {
        if self.client.config.save_responses {
            let report_path = self.client.config.output_dir.join("test_report.json");
            let report_json = serde_json::to_string_pretty(&self.report)?;
            fs::write(&report_path, &report_json)?;
            println!("  Report saved to: {}", report_path.display());
        }
        Ok(())
    }
}

// =============================================================================
// Public Entry Points
// =============================================================================

/// Run the GLEIF API test harness (original mode)
pub async fn run_gleif_tests(verbose: bool) -> Result<()> {
    let config = TestConfig {
        verbose,
        save_responses: true,
        ..Default::default()
    };

    let mut harness = GleifTestHarness::new(config).await?;
    harness.run_all_tests().await?;

    // Return error if there were failures
    if harness.report.summary.failed_endpoints > 0 {
        anyhow::bail!(
            "{} endpoint tests failed",
            harness.report.summary.failed_endpoints
        );
    }

    Ok(())
}

/// Run a full GLEIF crawl starting from a root LEI
pub async fn run_gleif_crawl(
    root_lei: Option<String>,
    max_depth: usize,
    max_entities: usize,
    dry_run: bool,
    verbose: bool,
) -> Result<CrawlStats> {
    let config = CrawlConfig {
        root_lei: root_lei.unwrap_or_else(|| ALLIANZ_SE_LEI.to_string()),
        max_depth,
        max_entities,
        dry_run,
        verbose,
        ..Default::default()
    };

    // Connect to database if not dry run
    let pool = if !dry_run {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql:///data_designer".to_string());
        Some(
            PgPool::connect(&db_url)
                .await
                .context("Failed to connect to database")?,
        )
    } else {
        None
    };

    let mut crawler = GleifCrawler::new(config, pool).await?;
    crawler.crawl().await
}
