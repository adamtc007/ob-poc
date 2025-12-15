//! HTTP API Client for ob-poc-ui
//!
//! All API calls are async and return Results.
//! Results are stored in AsyncState by the caller, then processed in update().

use ob_poc_graph::{CbuGraphData, ViewMode};
use ob_poc_types::{
    CbuSummary, ChatRequest, ChatResponse, CreateSessionRequest, CreateSessionResponse,
    ExecuteRequest, ExecuteResponse, SessionStateResponse, SetBindingRequest, SetBindingResponse,
    ValidateDslRequest, ValidateDslResponse,
};
use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, RequestMode, Response};

/// Base URL (same origin)
const BASE_URL: &str = "";

// =============================================================================
// Generic HTTP Methods
// =============================================================================

/// GET request returning JSON
pub async fn get<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let url = format!("{}{}", BASE_URL, path);

    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::SameOrigin);

    let request = Request::new_with_str_and_init(&url, &opts)
        .map_err(|e| format!("Request creation failed: {:?}", e))?;

    request
        .headers()
        .set("Accept", "application/json")
        .map_err(|e| format!("Header set failed: {:?}", e))?;

    let window = web_sys::window().ok_or("No window")?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch failed: {:?}", e))?;

    let resp: Response = resp_value.dyn_into().map_err(|_| "Response cast failed")?;

    if !resp.ok() {
        return Err(format!("HTTP {}: {}", resp.status(), resp.status_text()));
    }

    let json = JsFuture::from(
        resp.json()
            .map_err(|e| format!("JSON parse failed: {:?}", e))?,
    )
    .await
    .map_err(|e| format!("JSON await failed: {:?}", e))?;

    serde_wasm_bindgen::from_value(json.clone()).map_err(|e| {
        // On error, log response for debugging
        if let Ok(json_str) = js_sys::JSON::stringify(&json) {
            let s: String = json_str.into();
            web_sys::console::error_1(
                &format!("Deserialize failed for {}: {}\nResponse: {}", url, e, s).into(),
            );
        }
        format!("Deserialize failed: {:?}", e)
    })
}

/// POST request with JSON body returning JSON
pub async fn post<T: DeserializeOwned, B: Serialize>(path: &str, body: &B) -> Result<T, String> {
    let url = format!("{}{}", BASE_URL, path);

    let body_str = serde_json::to_string(body).map_err(|e| format!("Serialize failed: {:?}", e))?;

    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::SameOrigin);
    opts.set_body(&JsValue::from_str(&body_str));

    let headers = Headers::new().map_err(|e| format!("Headers creation failed: {:?}", e))?;
    headers.set("Content-Type", "application/json").ok();
    headers.set("Accept", "application/json").ok();
    opts.set_headers(&headers);

    let request = Request::new_with_str_and_init(&url, &opts)
        .map_err(|e| format!("Request creation failed: {:?}", e))?;

    let window = web_sys::window().ok_or("No window")?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch failed: {:?}", e))?;

    let resp: Response = resp_value.dyn_into().map_err(|_| "Response cast failed")?;

    if !resp.ok() {
        // Try to get error body
        let status = resp.status();
        let status_text = resp.status_text();
        return Err(format!("HTTP {}: {}", status, status_text));
    }

    let json = JsFuture::from(
        resp.json()
            .map_err(|e| format!("JSON parse failed: {:?}", e))?,
    )
    .await
    .map_err(|e| format!("JSON await failed: {:?}", e))?;

    serde_wasm_bindgen::from_value(json.clone()).map_err(|e| {
        // On error, log response for debugging
        if let Ok(json_str) = js_sys::JSON::stringify(&json) {
            let s: String = json_str.into();
            web_sys::console::error_1(
                &format!(
                    "Deserialize failed for POST {}: {}\nResponse: {}",
                    url, e, s
                )
                .into(),
            );
        }
        format!("Deserialize failed: {:?}", e)
    })
}

// =============================================================================
// Session API
// =============================================================================

/// Create a new session
pub async fn create_session() -> Result<CreateSessionResponse, String> {
    post("/api/session", &CreateSessionRequest { domain_hint: None }).await
}

/// Get session state
pub async fn get_session(session_id: Uuid) -> Result<SessionStateResponse, String> {
    get(&format!("/api/session/{}", session_id)).await
}

/// Bind an entity to the session context
pub async fn bind_entity(
    session_id: Uuid,
    entity_id: Uuid,
    entity_type: &str,
    display_name: &str,
) -> Result<SetBindingResponse, String> {
    post(
        &format!("/api/session/{}/bind", session_id),
        &SetBindingRequest {
            name: entity_type.to_string(), // Use entity_type as binding name (e.g., "cbu")
            id: entity_id.to_string(),
            entity_type: entity_type.to_string(),
            display_name: display_name.to_string(),
        },
    )
    .await
}

// =============================================================================
// Chat API
// =============================================================================

/// Send chat message (non-streaming)
pub async fn send_chat(session_id: Uuid, message: &str) -> Result<ChatResponse, String> {
    post(
        &format!("/api/session/{}/chat", session_id),
        &ChatRequest {
            message: message.to_string(),
            cbu_id: None,
        },
    )
    .await
}

// =============================================================================
// DSL API
// =============================================================================

/// Validate DSL syntax and semantics
/// Returns list of error strings (empty = valid)
pub async fn validate_dsl(dsl: &str) -> Result<Vec<String>, String> {
    let response: ValidateDslResponse = post(
        "/api/agent/validate",
        &ValidateDslRequest {
            dsl: dsl.to_string(),
        },
    )
    .await?;

    Ok(response.errors)
}

/// Execute DSL with explicit content (for manual DSL editor execution)
pub async fn execute_dsl(session_id: Uuid, dsl: &str) -> Result<ExecuteResponse, String> {
    post(
        &format!("/api/session/{}/execute", session_id),
        &ExecuteRequest {
            dsl: Some(dsl.to_string()),
        },
    )
    .await
}

/// Execute session's accumulated DSL (no DSL content passed - uses server state)
pub async fn execute_session(session_id: Uuid) -> Result<ExecuteResponse, String> {
    post(
        &format!("/api/session/{}/execute", session_id),
        &ExecuteRequest { dsl: None },
    )
    .await
}

// =============================================================================
// Graph API
// =============================================================================

/// Get CBU graph data
pub async fn get_cbu_graph(cbu_id: Uuid, view_mode: ViewMode) -> Result<CbuGraphData, String> {
    let view_mode_str = match view_mode {
        ViewMode::KycUbo => "KYC_UBO",
        ViewMode::ServiceDelivery => "SERVICE_DELIVERY",
        ViewMode::Custody => "CUSTODY",
        ViewMode::ProductsOnly => "PRODUCTS_ONLY",
    };
    get(&format!(
        "/api/cbu/{}/graph?view_mode={}",
        cbu_id, view_mode_str
    ))
    .await
}

// =============================================================================
// CBU API
// =============================================================================

/// List all CBUs
pub async fn list_cbus() -> Result<Vec<CbuSummary>, String> {
    get("/api/cbu").await
}

// =============================================================================
// Local Storage Helpers
// =============================================================================

/// Get value from localStorage
pub fn get_local_storage(key: &str) -> Option<String> {
    let window = web_sys::window()?;
    let storage = window.local_storage().ok()??;
    storage.get_item(key).ok()?
}

/// Set value in localStorage
pub fn set_local_storage(key: &str, value: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or("No window")?;
    let storage = window
        .local_storage()
        .map_err(|_| "No localStorage")?
        .ok_or("No localStorage")?;
    storage
        .set_item(key, value)
        .map_err(|_| "Failed to set localStorage item")?;
    Ok(())
}

/// Remove value from localStorage
pub fn remove_local_storage(key: &str) -> Result<(), String> {
    let window = web_sys::window().ok_or("No window")?;
    let storage = window
        .local_storage()
        .map_err(|_| "No localStorage")?
        .ok_or("No localStorage")?;
    storage
        .remove_item(key)
        .map_err(|_| "Failed to remove localStorage item")?;
    Ok(())
}
