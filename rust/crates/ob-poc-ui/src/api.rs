//! HTTP API Client for ob-poc-ui
//!
//! All API calls are async and return Results.
//! Results are stored in AsyncState by the caller, then processed in update().

use ob_poc_graph::{CbuGraphData, TradingMatrix, ViewMode};
use ob_poc_types::{
    galaxy::UniverseGraph,
    investor_register::{InvestorFilters, InvestorListResponse, InvestorRegisterView},
    CbuSummary, ChatRequest, ChatResponse, CommitResolutionResponse, ConfirmAllRequest,
    ConfirmResolutionRequest, CreateSessionRequest, CreateSessionResponse, ExecuteRequest,
    ExecuteResponse, GetContextResponse, ResolutionSearchRequest, ResolutionSearchResponse,
    ResolutionSessionResponse, SelectResolutionRequest, SelectResolutionResponse, SessionContext,
    SessionStateResponse, SetBindingRequest, SetBindingResponse, StartResolutionRequest,
    ValidateDslRequest, ValidateDslResponse,
};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
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
    web_sys::console::log_1(&format!("api::get: starting request to {}", url).into());

    let opts = RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(RequestMode::SameOrigin);
    web_sys::console::log_1(&"api::get: created RequestInit".into());

    let request = Request::new_with_str_and_init(&url, &opts)
        .map_err(|e| format!("Request creation failed: {:?}", e))?;
    web_sys::console::log_1(&"api::get: created Request".into());

    request
        .headers()
        .set("Accept", "application/json")
        .map_err(|e| format!("Header set failed: {:?}", e))?;
    web_sys::console::log_1(&"api::get: set headers".into());

    let window = web_sys::window().ok_or("No window")?;
    web_sys::console::log_1(&"api::get: got window, calling fetch...".into());

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch failed: {:?}", e))?;
    web_sys::console::log_1(&"api::get: fetch completed".into());

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

/// Get session version only (lightweight check for external changes)
/// Returns the version string from the session, used for polling
pub async fn get_session_version(session_id: Uuid) -> Result<String, String> {
    // For now, fetch full session and extract version
    // TODO: Add lightweight /api/session/:id/version endpoint for efficiency
    let session: SessionStateResponse = get(&format!("/api/session/{}", session_id)).await?;
    session
        .version
        .ok_or_else(|| "Session has no version".to_string())
}

// =============================================================================
// Session Watch API (Long-Polling for Changes)
// =============================================================================

/// Response from session watch endpoint
#[derive(Clone, Debug, serde::Deserialize)]
pub struct WatchSessionResponse {
    pub session_id: Uuid,
    pub version: u64,
    pub scope_path: String,
    pub has_mass: bool,
    pub view_mode: Option<String>,
    pub active_cbu_id: Option<Uuid>,
    pub updated_at: String,
    pub is_initial: bool,
    /// Session scope type (galaxy, book, cbu, jurisdiction, neighborhood, empty)
    #[serde(default)]
    pub scope_type: Option<String>,
    /// Whether scope data is fully loaded
    #[serde(default)]
    pub scope_loaded: bool,
}

/// Long-poll for session changes
/// Returns when session changes or timeout occurs
/// Uses the /api/session/:id/watch endpoint
pub async fn watch_session(
    session_id: Uuid,
    timeout_ms: Option<u64>,
) -> Result<WatchSessionResponse, String> {
    let timeout = timeout_ms.unwrap_or(30000);
    get(&format!(
        "/api/session/{}/watch?timeout_ms={}",
        session_id, timeout
    ))
    .await
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

/// Sync view mode to server session
/// This allows the server to know what visualization context the client is in
pub async fn set_view_mode(
    session_id: Uuid,
    view_mode: &str,
    view_level: Option<&str>,
) -> Result<(), String> {
    #[derive(Serialize)]
    struct SetViewModeRequest {
        view_mode: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        view_level: Option<String>,
    }

    let _response: serde_json::Value = post(
        &format!("/api/session/{}/view-mode", session_id),
        &SetViewModeRequest {
            view_mode: view_mode.to_string(),
            view_level: view_level.map(|s| s.to_string()),
        },
    )
    .await?;

    Ok(())
}

/// Get session context (CBU info, linked entities, symbols)
/// Used to populate ContextPanel and inform agent prompts
pub async fn get_session_context(session_id: Uuid) -> Result<SessionContext, String> {
    let response: GetContextResponse = get(&format!("/api/session/{}/context", session_id)).await?;
    Ok(response.context)
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
/// Returns ValidateDslResponse with errors and warnings
pub async fn validate_dsl(dsl: &str) -> Result<ValidateDslResponse, String> {
    post(
        "/api/agent/validate",
        &ValidateDslRequest {
            dsl: dsl.to_string(),
        },
    )
    .await
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
// Graph API (single source of truth - no delegation)
// =============================================================================

/// Get CBU graph data
pub async fn get_cbu_graph(cbu_id: Uuid, view_mode: ViewMode) -> Result<CbuGraphData, String> {
    let view_mode_str = view_mode.as_str();
    let url = format!("/api/cbu/{}/graph?view_mode={}", cbu_id, view_mode_str);

    // Use shared CbuGraphResponse type, then convert to CbuGraphData
    let response: ob_poc_types::CbuGraphResponse = get(&url).await?;
    Ok(response.into())
}

// =============================================================================
// Galaxy Navigation API
// =============================================================================

/// Get universe graph (all clusters for galaxy view)
pub async fn get_universe_graph() -> Result<UniverseGraph, String> {
    get("/api/universe").await
}

/// Get universe graph filtered by commercial client (book view)
/// Maps to `view.book :client <name>` DSL verb
pub async fn get_universe_graph_by_client(client_name: &str) -> Result<UniverseGraph, String> {
    // Simple URL encoding for spaces and special chars
    let encoded = client_name
        .replace(' ', "%20")
        .replace('&', "%26")
        .replace('=', "%3D");
    get(&format!(
        "/api/universe?cluster_by=client&client={}",
        encoded
    ))
    .await
}

/// Get trading matrix (hierarchical custody configuration) for a CBU
pub async fn get_trading_matrix(cbu_id: Uuid) -> Result<TradingMatrix, String> {
    use ob_poc_graph::TradingMatrixResponse;

    // The API returns TradingMatrixResponse which we convert to TradingMatrix
    let response: TradingMatrixResponse =
        get(&format!("/api/cbu/{}/trading-matrix", cbu_id)).await?;

    // Convert response to UI wrapper
    Ok(TradingMatrix::from_response(response))
}

// =============================================================================
// CBU API
// =============================================================================

/// List all CBUs (limited to 50 for initial load)
pub async fn list_cbus() -> Result<Vec<CbuSummary>, String> {
    get("/api/cbu").await
}

/// Search CBUs using EntityGateway fuzzy search
/// Returns matches with scores for ranking
pub async fn search_cbus(query: &str, limit: u32) -> Result<CbuSearchResponse, String> {
    let encoded = js_sys::encode_uri_component(query);
    get(&format!(
        "/api/entity/search?type=cbu&q={}&limit={}",
        encoded, limit
    ))
    .await
}

/// CBU search response from entity search API
#[derive(Clone, Debug, serde::Deserialize)]
pub struct CbuSearchResponse {
    pub matches: Vec<CbuSearchMatch>,
    pub total: usize,
    pub truncated: bool,
}

/// Individual CBU match from fuzzy search
#[derive(Clone, Debug, serde::Deserialize)]
pub struct CbuSearchMatch {
    /// CBU UUID
    pub value: String,
    /// Display name
    pub display: String,
    /// Additional context (e.g., jurisdiction)
    pub detail: Option<String>,
    /// Relevance score 0.0-1.0
    pub score: f32,
}

// =============================================================================
// Stage Focus API
// =============================================================================

/// Request to set stage focus
#[derive(Clone, Debug, serde::Serialize)]
pub struct SetFocusRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage_code: Option<String>,
}

/// Response from setting stage focus
#[derive(Clone, Debug, serde::Deserialize)]
#[allow(dead_code)] // Fields are part of API response, may be used in future
pub struct SetFocusResponse {
    pub success: bool,
    pub stage_code: Option<String>,
    pub stage_name: Option<String>,
    pub relevant_verbs: Vec<String>,
}

/// Set or clear stage focus for the session
pub async fn set_stage_focus(
    session_id: Uuid,
    stage_code: Option<&str>,
) -> Result<SetFocusResponse, String> {
    post(
        &format!("/api/session/{}/focus", session_id),
        &SetFocusRequest {
            stage_code: stage_code.map(|s| s.to_string()),
        },
    )
    .await
}

// =============================================================================
// Resolution API
// =============================================================================

/// Start entity resolution for session's DSL
pub async fn start_resolution(session_id: Uuid) -> Result<ResolutionSessionResponse, String> {
    post(
        &format!("/api/session/{}/resolution/start", session_id),
        &StartResolutionRequest { ref_ids: None },
    )
    .await
}

/// Get current resolution state
#[allow(dead_code)]
pub async fn get_resolution(session_id: Uuid) -> Result<ResolutionSessionResponse, String> {
    get(&format!("/api/session/{}/resolution", session_id)).await
}

/// Search for entity matches for a specific ref
pub async fn search_resolution(
    session_id: Uuid,
    ref_id: &str,
    query: &str,
    discriminators: HashMap<String, String>,
) -> Result<ResolutionSearchResponse, String> {
    post(
        &format!("/api/session/{}/resolution/search", session_id),
        &ResolutionSearchRequest {
            ref_id: ref_id.to_string(),
            query: query.to_string(),
            discriminators,
            limit: Some(10),
        },
    )
    .await
}

/// Select an entity for a ref
pub async fn select_resolution(
    session_id: Uuid,
    ref_id: &str,
    resolved_key: &str,
) -> Result<SelectResolutionResponse, String> {
    post(
        &format!("/api/session/{}/resolution/select", session_id),
        &SelectResolutionRequest {
            ref_id: ref_id.to_string(),
            resolved_key: resolved_key.to_string(),
        },
    )
    .await
}

/// Confirm a resolution (mark as reviewed)
#[allow(dead_code)]
pub async fn confirm_resolution(
    session_id: Uuid,
    ref_id: &str,
) -> Result<ResolutionSessionResponse, String> {
    post(
        &format!("/api/session/{}/resolution/confirm", session_id),
        &ConfirmResolutionRequest {
            ref_id: ref_id.to_string(),
        },
    )
    .await
}

/// Confirm all high-confidence resolutions
pub async fn confirm_all_resolutions(
    session_id: Uuid,
    min_confidence: Option<f32>,
) -> Result<ResolutionSessionResponse, String> {
    post(
        &format!("/api/session/{}/resolution/confirm-all", session_id),
        &ConfirmAllRequest { min_confidence },
    )
    .await
}

/// Commit resolutions to AST
pub async fn commit_resolution(session_id: Uuid) -> Result<CommitResolutionResponse, String> {
    post(
        &format!("/api/session/{}/resolution/commit", session_id),
        &(),
    )
    .await
}

/// Cancel resolution session
pub async fn cancel_resolution(session_id: Uuid) -> Result<(), String> {
    post::<(), _>(
        &format!("/api/session/{}/resolution/cancel", session_id),
        &(),
    )
    .await
}

// =============================================================================
// Taxonomy Navigation API
// =============================================================================

/// Breadcrumb entry from taxonomy navigation
#[derive(Clone, Debug, serde::Deserialize)]
pub struct Breadcrumb {
    pub label: String,
    pub type_code: String,
    pub index: usize,
}

/// Response from taxonomy breadcrumbs endpoint
#[derive(Clone, Debug, serde::Deserialize)]
pub struct TaxonomyBreadcrumbsResponse {
    pub breadcrumbs: Vec<Breadcrumb>,
    pub depth: usize,
    pub can_zoom_out: bool,
}

/// Response from taxonomy zoom operations
#[derive(Clone, Debug, serde::Deserialize)]
pub struct TaxonomyZoomResponse {
    pub success: bool,
    pub breadcrumbs: Vec<Breadcrumb>,
    pub depth: usize,
    pub error: Option<String>,
}

/// Request for zoom-in operation
#[derive(Clone, Debug, serde::Serialize)]
struct ZoomInRequest {
    type_code: String,
}

/// Request for back-to operation
#[derive(Clone, Debug, serde::Serialize)]
struct BackToRequest {
    level_index: usize,
}

/// Get current taxonomy breadcrumbs for a session
pub async fn get_taxonomy_breadcrumbs(
    session_id: Uuid,
) -> Result<TaxonomyBreadcrumbsResponse, String> {
    get(&format!("/api/session/{}/taxonomy/breadcrumbs", session_id)).await
}

/// Zoom into a type (push onto taxonomy stack)
pub async fn taxonomy_zoom_in(
    session_id: Uuid,
    type_code: &str,
) -> Result<TaxonomyZoomResponse, String> {
    post(
        &format!("/api/session/{}/taxonomy/zoom-in", session_id),
        &ZoomInRequest {
            type_code: type_code.to_string(),
        },
    )
    .await
}

/// Zoom out one level (pop from taxonomy stack)
pub async fn taxonomy_zoom_out(session_id: Uuid) -> Result<TaxonomyZoomResponse, String> {
    post(
        &format!("/api/session/{}/taxonomy/zoom-out", session_id),
        &(),
    )
    .await
}

/// Jump to a specific breadcrumb level
pub async fn taxonomy_back_to(
    session_id: Uuid,
    level_index: usize,
) -> Result<TaxonomyZoomResponse, String> {
    post(
        &format!("/api/session/{}/taxonomy/back-to", session_id),
        &BackToRequest { level_index },
    )
    .await
}

/// Reset taxonomy to root level
pub async fn taxonomy_reset(session_id: Uuid) -> Result<TaxonomyZoomResponse, String> {
    post(&format!("/api/session/{}/taxonomy/reset", session_id), &()).await
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
#[allow(dead_code)]
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

// =============================================================================
// Investor Register API
// =============================================================================

/// Get investor register view for an issuer
/// Returns control holders (>5%) as individual nodes and aggregate for others
pub async fn get_investor_register(
    issuer_id: &str,
    share_class: Option<&str>,
) -> Result<InvestorRegisterView, String> {
    let mut url = format!("/api/capital/{}/investors", issuer_id);
    if let Some(sc) = share_class {
        let encoded = js_sys::encode_uri_component(sc);
        url = format!("{}?share_class={}", url, encoded);
    }
    get(&url).await
}

/// Get paginated investor list for drill-down
pub async fn get_investor_list(
    issuer_id: &str,
    page: i32,
    page_size: i32,
    filters: &InvestorFilters,
) -> Result<InvestorListResponse, String> {
    let mut params = vec![format!("page={}", page), format!("page_size={}", page_size)];

    if let Some(ref t) = filters.investor_type {
        params.push(format!("investor_type={}", js_sys::encode_uri_component(t)));
    }
    if let Some(ref s) = filters.kyc_status {
        params.push(format!("kyc_status={}", js_sys::encode_uri_component(s)));
    }
    if let Some(ref j) = filters.jurisdiction {
        params.push(format!("jurisdiction={}", js_sys::encode_uri_component(j)));
    }
    if let Some(ref q) = filters.search {
        params.push(format!("search={}", js_sys::encode_uri_component(q)));
    }
    if let Some(min) = filters.min_units {
        params.push(format!("min_units={}", min));
    }

    let url = format!(
        "/api/capital/{}/investors/list?{}",
        issuer_id,
        params.join("&")
    );
    get(&url).await
}
