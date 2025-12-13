//! API client for graph data
//!
//! Uses shared types from ob-poc-types for API responses.
//! Uses web-sys fetch for WASM, reqwest for native.

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

    #[cfg(target_arch = "wasm32")]
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::JsFuture;
        use web_sys::{Request, RequestInit, RequestMode, Response};

        let url = format!("{}{}", self.base_url, path);

        let opts = RequestInit::new();
        opts.set_method("GET");
        opts.set_mode(RequestMode::Cors);

        let request = Request::new_with_str_and_init(&url, &opts)
            .map_err(|e| format!("request error: {:?}", e))?;

        let window = web_sys::window().ok_or("no window")?;
        let resp_value = JsFuture::from(window.fetch_with_request(&request))
            .await
            .map_err(|e| format!("fetch error: {:?}", e))?;

        let resp: Response = resp_value
            .dyn_into()
            .map_err(|_| "response is not a Response")?;

        if !resp.ok() {
            return Err(format!("HTTP {}", resp.status()));
        }

        let json = JsFuture::from(
            resp.json()
                .map_err(|e| format!("json promise error: {:?}", e))?,
        )
        .await
        .map_err(|e| format!("json error: {:?}", e))?;

        let data: T = serde_wasm_bindgen::from_value(json)
            .map_err(|e| format!("deserialize error: {}", e))?;

        Ok(data)
    }

    #[cfg(not(target_arch = "wasm32"))]
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
