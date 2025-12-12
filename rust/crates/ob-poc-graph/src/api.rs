//! API client for graph data
//!
//! Uses shared types from ob-poc-types for API responses.

use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::graph::ViewMode;
// Use shared API types (single source of truth)
use ob_poc_types::CbuGraphResponse;

#[derive(Clone)]
pub struct ApiClient {
    base_url: String,
}

impl ApiClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
        }
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);

        let response = reqwest::get(&url).await.map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }

        response.json::<T>().await.map_err(|e| e.to_string())
    }

    /// Fetch CBU graph data using shared API types
    pub async fn get_cbu_graph(
        &self,
        cbu_id: Uuid,
        view_mode: ViewMode,
    ) -> Result<CbuGraphResponse, String> {
        let path = format!("/api/cbu/{}/graph?view_mode={}", cbu_id, view_mode.as_str());
        self.get(&path).await
    }
}
