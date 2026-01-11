//! SEC EDGAR API client
//!
//! Rate-limited HTTP client for SEC EDGAR data.
//!
//! # Important
//!
//! SEC EDGAR requires a User-Agent header with contact info.
//! Rate limit is 10 requests per second.

use super::types::{SecCompanySubmissions, SecFilingInfo};
use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::time::sleep;

const SEC_API_BASE: &str = "https://data.sec.gov";
const SEC_ARCHIVES_BASE: &str = "https://www.sec.gov/Archives/edgar/data";
const RATE_LIMIT_DELAY_MS: u64 = 100; // 10 req/sec

/// SEC EDGAR API client
pub struct SecEdgarClient {
    http: Client,
    last_request: Mutex<Instant>,
}

impl SecEdgarClient {
    /// Create a new client
    pub fn new() -> Result<Self> {
        // SEC requires a User-Agent with contact info
        let user_agent = std::env::var("SEC_EDGAR_USER_AGENT")
            .unwrap_or_else(|_| "OB-POC/1.0 (compliance@example.com)".to_string());

        let http = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(user_agent)
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            http,
            last_request: Mutex::new(Instant::now()),
        })
    }

    /// Enforce rate limiting
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

    /// Get company submissions by CIK
    pub async fn get_company(&self, cik: &str) -> Result<SecCompanySubmissions> {
        self.rate_limit().await;

        let cik_padded = pad_cik(cik);
        let url = format!("{}/submissions/CIK{}.json", SEC_API_BASE, cik_padded);

        let response = self
            .http
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch SEC submissions for CIK {}", cik))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "SEC EDGAR API error {}: {}",
                status,
                body.chars().take(200).collect::<String>()
            ));
        }

        response
            .json()
            .await
            .with_context(|| format!("Failed to parse SEC response for CIK {}", cik))
    }

    /// Get 13D/13G filings for a company
    pub async fn get_beneficial_ownership_filings(&self, cik: &str) -> Result<Vec<SecFilingInfo>> {
        let submissions = self.get_company(cik).await?;
        Ok(submissions.filings.recent.beneficial_ownership_filings())
    }

    /// Fetch a filing document
    pub async fn fetch_filing_document(
        &self,
        cik: &str,
        accession_number: &str,
        document: &str,
    ) -> Result<String> {
        self.rate_limit().await;

        let cik_padded = pad_cik(cik);
        let accession_clean = accession_number.replace('-', "");
        let url = format!(
            "{}/{}/{}/{}",
            SEC_ARCHIVES_BASE, cik_padded, accession_clean, document
        );

        let response = self
            .http
            .get(&url)
            .send()
            .await
            .with_context(|| format!("Failed to fetch filing document {}", document))?;

        let status = response.status();
        if !status.is_success() {
            return Err(anyhow!("SEC EDGAR error {}: {}", status, url));
        }

        response
            .text()
            .await
            .with_context(|| format!("Failed to read filing document {}", document))
    }
}

impl Default for SecEdgarClient {
    fn default() -> Self {
        Self::new().expect("Failed to create SEC EDGAR client")
    }
}

/// Pad CIK to 10 digits
fn pad_cik(cik: &str) -> String {
    let digits_only = cik.trim().trim_start_matches('0');
    format!("{:0>10}", digits_only)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pad_cik() {
        assert_eq!(pad_cik("320193"), "0000320193");
        assert_eq!(pad_cik("0000320193"), "0000320193");
        assert_eq!(pad_cik("1234567890"), "1234567890");
        assert_eq!(pad_cik("1"), "0000000001");
    }
}
