//! Companies House API client
//!
//! Rate-limited HTTP client for the UK Companies House API.

use super::types::{ChCompanyProfile, ChOfficerList, ChPscList, ChSearchResult};
use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::time::sleep;

const CH_API_BASE: &str = "https://api.company-information.service.gov.uk";
const RATE_LIMIT_DELAY_MS: u64 = 500; // ~2 req/sec to stay well under 600/5min

/// Companies House API client
pub struct CompaniesHouseClient {
    http: Client,
    api_key: String,
    last_request: Mutex<Instant>,
}

impl CompaniesHouseClient {
    /// Create a new client from environment variable
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("COMPANIES_HOUSE_API_KEY")
            .context("COMPANIES_HOUSE_API_KEY environment variable not set")?;
        Self::new(api_key)
    }

    /// Create a new client with the given API key
    pub fn new(api_key: String) -> Result<Self> {
        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            http,
            api_key,
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

    /// Make a GET request with authentication
    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.rate_limit().await;

        let url = format!("{}{}", CH_API_BASE, path);

        let response = self
            .http
            .get(&url)
            .basic_auth(&self.api_key, Option::<&str>::None)
            .header("Accept", "application/json")
            .send()
            .await
            .with_context(|| format!("Failed to fetch {}", path))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Companies House API error {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            ));
        }

        response
            .json()
            .await
            .with_context(|| format!("Failed to parse response from {}", path))
    }

    /// Get company profile by company number
    pub async fn get_company(&self, number: &str) -> Result<ChCompanyProfile> {
        let number = normalize_company_number(number);
        self.get(&format!("/company/{}", number)).await
    }

    /// Get Persons with Significant Control for a company
    pub async fn get_psc(&self, number: &str) -> Result<ChPscList> {
        let number = normalize_company_number(number);
        self.get(&format!(
            "/company/{}/persons-with-significant-control",
            number
        ))
        .await
    }

    /// Get officers for a company
    pub async fn get_officers(&self, number: &str) -> Result<ChOfficerList> {
        let number = normalize_company_number(number);
        self.get(&format!("/company/{}/officers", number)).await
    }

    /// Search for companies by name
    pub async fn search(&self, query: &str, limit: usize) -> Result<ChSearchResult> {
        let encoded_query = encode_query_param(query);
        self.get(&format!(
            "/search/companies?q={}&items_per_page={}",
            encoded_query, limit
        ))
        .await
    }
}

/// Simple URL encoding for query parameters
fn encode_query_param(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ' ' => "%20".to_string(),
            '&' => "%26".to_string(),
            '=' => "%3D".to_string(),
            '+' => "%2B".to_string(),
            '#' => "%23".to_string(),
            '%' => "%25".to_string(),
            '?' => "%3F".to_string(),
            c if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' => {
                c.to_string()
            }
            c => format!("%{:02X}", c as u32),
        })
        .collect()
}

/// Normalize company number to 8 characters with leading zeros
fn normalize_company_number(number: &str) -> String {
    let number = number.trim().to_uppercase();

    // Handle Scottish, Northern Irish, and other prefixed numbers
    if number.starts_with("SC")
        || number.starts_with("NI")
        || number.starts_with("NC")
        || number.starts_with("NF")
        || number.starts_with("OC")
        || number.starts_with("SO")
        || number.starts_with("LP")
        || number.starts_with("SL")
        || number.starts_with("FC")
        || number.starts_with("SF")
        || number.starts_with("NL")
        || number.starts_with("GE")
        || number.starts_with("IP")
        || number.starts_with("SP")
        || number.starts_with("IC")
        || number.starts_with("SI")
        || number.starts_with("NP")
        || number.starts_with("NO")
        || number.starts_with("RC")
        || number.starts_with("SR")
        || number.starts_with("AC")
        || number.starts_with("SA")
        || number.starts_with("NA")
        || number.starts_with("NZ")
        || number.starts_with("CE")
        || number.starts_with("CS")
        || number.starts_with("PC")
        || number.starts_with("RS")
    {
        // Prefix + 6 digits = 8 chars
        if number.len() < 8 {
            let prefix = &number[..2];
            let digits = &number[2..];
            return format!("{}{:0>6}", prefix, digits);
        }
        return number;
    }

    // Pure numeric - pad to 8 digits
    if number.chars().all(|c| c.is_ascii_digit()) {
        return format!("{:0>8}", number);
    }

    // Return as-is if we can't normalize
    number
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_company_number() {
        // Pure numeric
        assert_eq!(normalize_company_number("12345678"), "12345678");
        assert_eq!(normalize_company_number("1234567"), "01234567");
        assert_eq!(normalize_company_number("123456"), "00123456");

        // Scottish companies
        assert_eq!(normalize_company_number("SC123456"), "SC123456");
        assert_eq!(normalize_company_number("SC12345"), "SC012345");
        assert_eq!(normalize_company_number("sc123456"), "SC123456");

        // Northern Irish
        assert_eq!(normalize_company_number("NI123456"), "NI123456");
        assert_eq!(normalize_company_number("NI1234"), "NI001234");

        // LLPs
        assert_eq!(normalize_company_number("OC123456"), "OC123456");
    }
}
