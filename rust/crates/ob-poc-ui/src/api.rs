//! API client for backend communication
//!
//! Provides typed methods for all backend API endpoints:
//! - Session management (create, chat, execute, bind)
//! - Entity search
//! - DSL operations (parse, resolve-ref)
//! - CBU graph data

#![allow(dead_code)]

use crate::state::*;
use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

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

    // =========================================================================
    // GENERIC HTTP METHODS
    // =========================================================================

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);

        let response = reqwest::get(&url).await.map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }

        response.json::<T>().await.map_err(|e| e.to_string())
    }

    pub async fn post<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, String> {
        let url = format!("{}{}", self.base_url, path);

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(format!("HTTP {}: {}", status, text));
        }

        response.json::<T>().await.map_err(|e| e.to_string())
    }

    pub async fn delete(&self, path: &str) -> Result<(), String> {
        let url = format!("{}{}", self.base_url, path);

        let client = reqwest::Client::new();
        let response = client
            .delete(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(format!("HTTP {}", status));
        }

        Ok(())
    }

    // =========================================================================
    // SESSION MANAGEMENT
    // =========================================================================

    /// Create a new agent session
    pub async fn create_session(&self) -> Result<SessionResponse, String> {
        #[derive(Serialize)]
        struct CreateSessionRequest {}

        self.post("/api/session", &CreateSessionRequest {}).await
    }

    /// Send a chat message to the session
    pub async fn chat(&self, session_id: Uuid, message: &str) -> Result<ChatResponse, String> {
        #[derive(Serialize)]
        struct ChatRequest<'a> {
            message: &'a str,
        }

        let path = format!("/api/session/{}/chat", session_id);
        self.post(&path, &ChatRequest { message }).await
    }

    /// Execute pending DSL in the session
    pub async fn execute(
        &self,
        session_id: Uuid,
        dsl: Option<&str>,
    ) -> Result<ExecuteResponse, String> {
        #[derive(Serialize)]
        struct ExecuteRequest<'a> {
            #[serde(skip_serializing_if = "Option::is_none")]
            dsl: Option<&'a str>,
        }

        let path = format!("/api/session/{}/execute", session_id);
        self.post(&path, &ExecuteRequest { dsl }).await
    }

    /// Bind a symbol in the session
    pub async fn bind(&self, session_id: Uuid, binding: &BindRequest) -> Result<(), String> {
        let path = format!("/api/session/{}/bind", session_id);
        let _: serde_json::Value = self.post(&path, binding).await?;
        Ok(())
    }

    /// Clear the session DSL
    pub async fn clear_session(&self, session_id: Uuid) -> Result<(), String> {
        let path = format!("/api/session/{}/clear", session_id);
        #[derive(Serialize)]
        struct Empty {}
        let _: serde_json::Value = self.post(&path, &Empty {}).await?;
        Ok(())
    }

    /// Delete a session
    pub async fn delete_session(&self, session_id: Uuid) -> Result<(), String> {
        let path = format!("/api/session/{}", session_id);
        self.delete(&path).await
    }

    // =========================================================================
    // ENTITY SEARCH
    // =========================================================================

    /// Search for entities by type and query
    pub async fn search_entities(
        &self,
        entity_type: &str,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<EntityMatch>, String> {
        let limit = limit.unwrap_or(10);
        let path = format!(
            "/api/entity/search?type={}&q={}&limit={}",
            urlencoding::encode(entity_type),
            urlencoding::encode(query),
            limit
        );

        // API returns { matches: [...] } or { results: [...] }
        #[derive(serde::Deserialize)]
        struct SearchResponse {
            #[serde(default, alias = "results")]
            matches: Vec<EntityMatch>,
        }

        let resp: SearchResponse = self.get(&path).await?;
        Ok(resp.matches)
    }

    /// Get completions for entity type (used by CBU picker)
    pub async fn complete(
        &self,
        entity_type: &str,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<CompletionItem>, String> {
        let req = CompleteRequest {
            entity_type: entity_type.to_string(),
            query: query.to_string(),
            limit,
        };
        let resp: CompleteResponse = self.post("/api/agent/complete", &req).await?;
        Ok(resp.items)
    }

    // =========================================================================
    // DSL OPERATIONS
    // =========================================================================

    /// Parse DSL source into AST
    pub async fn parse_dsl(&self, dsl: &str) -> Result<ParseResponse, String> {
        #[derive(Serialize)]
        struct ParseRequest<'a> {
            dsl: &'a str,
        }

        self.post("/api/dsl/parse", &ParseRequest { dsl }).await
    }

    /// Resolve an EntityRef by its ID
    pub async fn resolve_ref(
        &self,
        dsl: &str,
        ref_id: RefId,
        resolved_key: &str,
    ) -> Result<ResolveRefResponse, String> {
        let req = ResolveRefRequest {
            dsl: dsl.to_string(),
            ref_id,
            resolved_key: resolved_key.to_string(),
        };
        self.post("/api/dsl/resolve-ref", &req).await
    }

    // =========================================================================
    // CBU & GRAPH
    // =========================================================================

    /// List all CBUs
    pub async fn list_cbus(&self) -> Result<Vec<CbuSummary>, String> {
        self.get("/api/cbu").await
    }

    /// Get CBU graph with view mode and orientation
    pub async fn get_cbu_graph(
        &self,
        cbu_id: Uuid,
        view_mode: ViewMode,
        orientation: Orientation,
    ) -> Result<crate::graph::CbuGraphData, String> {
        let path = format!(
            "/api/cbu/{}/graph?view_mode={}&orientation={}",
            cbu_id,
            view_mode.as_str(),
            orientation.as_str()
        );
        self.get(&path).await
    }

    /// Get layout overrides for a CBU
    pub async fn get_layout(
        &self,
        cbu_id: Uuid,
        view_mode: ViewMode,
    ) -> Result<crate::graph::LayoutOverride, String> {
        let path = format!(
            "/api/cbu/{}/layout?view_mode={}",
            cbu_id,
            view_mode.as_str()
        );
        self.get(&path).await
    }

    /// Save layout overrides for a CBU
    pub async fn save_layout(
        &self,
        cbu_id: Uuid,
        view_mode: ViewMode,
        overrides: &crate::graph::LayoutOverride,
    ) -> Result<(), String> {
        let path = format!(
            "/api/cbu/{}/layout?view_mode={}",
            cbu_id,
            view_mode.as_str()
        );
        let _: crate::graph::LayoutOverride = self.post(&path, overrides).await?;
        Ok(())
    }
}

// URL encoding helper
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::new();
        for c in s.chars() {
            match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                    result.push(c);
                }
                ' ' => result.push_str("%20"),
                _ => {
                    for b in c.to_string().bytes() {
                        result.push_str(&format!("%{:02X}", b));
                    }
                }
            }
        }
        result
    }
}
