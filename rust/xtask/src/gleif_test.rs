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

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::time::sleep;

// =============================================================================
// Test LEIs - Curated for different scenarios
// =============================================================================

/// Allianz SE (Head Office) - Large corporate with many children
pub const ALLIANZ_SE_LEI: &str = "529900K9B0N5BT694847";

/// Allianz Global Investors GmbH (ManCo) - Fund manager with managed funds
pub const ALLIANZ_GI_LEI: &str = "529900FAHFDMSXCPII15";

// =============================================================================
// Configuration
// =============================================================================

const GLEIF_API_BASE: &str = "https://api.gleif.org/api/v1";
const RATE_LIMIT_DELAY_MS: u64 = 250; // Conservative rate limiting
const OUTPUT_DIR: &str = "data/gleif_test_output";

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
