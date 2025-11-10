//! Mock REST API Server for DSL Visualizer
//!
//! This module provides a mock REST API server that serves test data to the egui visualizer
//! without requiring database connectivity. This allows for testing the visualizer frontend
//! before the full database integration is complete.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};

use std::collections::HashMap;
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, warn};

/// Mock REST API server configuration
#[derive(Debug, Clone)]
pub struct MockRestApiConfig {
    pub host: String,
    pub port: u16,
}

impl Default for MockRestApiConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
        }
    }
}

/// Mock application state
#[derive(Clone)]
pub struct MockAppState {
    pub mock_data: Arc<MockData>,
}

/// Query parameters for DSL listing
#[derive(Debug, Deserialize)]
pub struct DslListQuery {
    pub search: Option<String>,
    pub domain: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// Response model for DSL entry in the list
#[derive(Debug, Serialize, Clone)]
pub struct DslEntryResponse {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub version: u32,
    pub description: String,
    pub created_at: String,
    pub status: String,
}

/// Response model for DSL list
#[derive(Debug, Serialize)]
pub struct DslListResponse {
    pub entries: Vec<DslEntryResponse>,
    pub total_count: u32,
}

/// Response model for DSL content with AST
#[derive(Debug, Serialize, Clone)]
pub struct DslContentResponse {
    pub id: String,
    pub content: String,
    pub ast: AstNodeResponse,
    pub version: u32,
    pub domain: String,
    pub status: String,
}

/// Response model for AST nodes
#[derive(Debug, Serialize, Clone)]
pub struct AstNodeResponse {
    pub id: String,
    pub node_type: String,
    pub label: String,
    pub properties: HashMap<String, String>,
    pub children: Vec<AstNodeResponse>,
    pub position: Option<(f32, f32)>,
}

/// Mock data container
pub struct MockData {
    pub dsl_entries: Vec<DslEntryResponse>,
    pub dsl_contents: HashMap<String, DslContentResponse>,
}

impl Default for MockData {
    fn default() -> Self {
        Self::new()
    }
}

impl MockData {
    pub fn new() -> Self {
        let entries = vec![
            DslEntryResponse {
                id: "dsl-001".to_string(),
                name: "Zenith Capital UBO Discovery".to_string(),
                domain: "UBO".to_string(),
                version: 1,
                description: "Ultimate Beneficial Ownership discovery workflow for Zenith Capital"
                    .to_string(),
                created_at: "2024-11-09T10:30:00Z".to_string(),
                status: "COMPILED".to_string(),
            },
            DslEntryResponse {
                id: "dsl-002".to_string(),
                name: "UCITS Fund Onboarding".to_string(),
                domain: "Onboarding".to_string(),
                version: 2,
                description: "UCITS equity fund onboarding and setup process".to_string(),
                created_at: "2024-11-08T14:20:00Z".to_string(),
                status: "FINALIZED".to_string(),
            },
            DslEntryResponse {
                id: "dsl-003".to_string(),
                name: "KYC Risk Assessment".to_string(),
                domain: "KYC".to_string(),
                version: 1,
                description: "Enhanced KYC risk assessment for high-value clients".to_string(),
                created_at: "2024-11-07T09:15:00Z".to_string(),
                status: "EDITING".to_string(),
            },
            DslEntryResponse {
                id: "dsl-004".to_string(),
                name: "Hedge Fund Investor Subscription".to_string(),
                domain: "Onboarding".to_string(),
                version: 3,
                description: "Hedge fund investor subscription and documentation workflow"
                    .to_string(),
                created_at: "2024-11-06T16:45:00Z".to_string(),
                status: "COMPILED".to_string(),
            },
            DslEntryResponse {
                id: "dsl-005".to_string(),
                name: "Corporate Banking Setup".to_string(),
                domain: "Onboarding".to_string(),
                version: 1,
                description: "Corporate banking account opening with trade finance".to_string(),
                created_at: "2024-11-05T11:30:00Z".to_string(),
                status: "CREATED".to_string(),
            },
        ];

        let mut contents = HashMap::new();

        // Mock content for Zenith Capital UBO
        contents.insert(
            "dsl-001".to_string(),
            DslContentResponse {
                id: "dsl-001".to_string(),
                content: r#"(define-kyc-investigation "zenith-capital-ubo-discovery"
  :target-entity "company-zenith-spv-001"
  :jurisdiction "KY"
  :ubo-threshold 25.0

  (declare-entity
    :node-id "company-zenith-spv-001"
    :label Company
    :properties {
      :legal-name "Zenith Capital Partners LP"
      :registration-number "KY-123456"
      :jurisdiction "KY"
    })

  (create-edge
    :from "alpha-holdings-sg"
    :to "company-zenith-spv-001"
    :type HAS_OWNERSHIP
    :properties {
      :percent 45.0
      :share-class "Class A Ordinary"
    }
    :evidenced-by ["doc-cayman-registry-001"])

  (validate customer.email_primary "john@zenithcapital.com")
  (collect kyc.risk_rating :from "risk-engine")
  (check compliance.fatca_status :equals "NON_US")
  (workflow.transition "UBO_DISCOVERY_COMPLETE"))"#
                    .to_string(),
                ast: create_zenith_ast(),
                version: 1,
                domain: "UBO".to_string(),
                status: "COMPILED".to_string(),
            },
        );

        // Mock content for UCITS Fund
        contents.insert(
            "dsl-002".to_string(),
            DslContentResponse {
                id: "dsl-002".to_string(),
                content: r#"(case.create
  (cbu.id "CBU-UCITS-001")
  (nature-purpose "UCITS equity fund domiciled in LU"))

(products.add "CUSTODY" "FUND_ACCOUNTING" "REGULATORY_REPORTING")

(kyc.start
  (documents (document "CertificateOfIncorporation" "CSSF_License"))
  (jurisdictions (jurisdiction "LU")))

(services.plan
  (service "Settlement" (sla "T+1"))
  (service "ValuationEngine" (frequency "daily"))
  (service "RegulatoryReporting" (frameworks ["UCITS_V" "AIFMD"])))

(resources.plan
  (resource "CustodyAccount" (owner "CustodyTech"))
  (resource "FundAccountingSystem" (owner "AccountingTech"))
  (resource "ComplianceMonitoring" (owner "ComplianceTeam")))

(workflow.transition "ONBOARDING_COMPLETE")"#
                    .to_string(),
                ast: create_ucits_ast(),
                version: 2,
                domain: "Onboarding".to_string(),
                status: "FINALIZED".to_string(),
            },
        );

        // Mock content for KYC Risk Assessment
        contents.insert(
            "dsl-003".to_string(),
            DslContentResponse {
                id: "dsl-003".to_string(),
                content: r#"(kyc.assessment "high-value-client-001"
  :client-type "individual"
  :risk-category "high"

  (collect.documents
    (passport :status "verified")
    (proof-of-address :status "pending")
    (source-of-wealth :type "employment" :documentation "required"))

  (screening.sanctions
    :databases ["OFAC" "EU_SANCTIONS" "UN_SANCTIONS"]
    :match-threshold 85.0)

  (screening.pep
    :databases ["WORLD_CHECK" "REFINITIV"]
    :relationship-depth 2)

  (risk.scoring
    :factors ["geography" "occupation" "transaction-patterns"]
    :model "enhanced-v2.1")

  (approval.required
    :level "senior-compliance-officer"
    :reason "high-risk-classification"))"#
                    .to_string(),
                ast: create_kyc_ast(),
                version: 1,
                domain: "KYC".to_string(),
                status: "EDITING".to_string(),
            },
        );

        Self {
            dsl_entries: entries,
            dsl_contents: contents,
        }
    }
}

/// Mock REST API server
pub struct MockRestApiServer {
    config: MockRestApiConfig,
    app_state: MockAppState,
}

impl MockRestApiServer {
    /// Create a new mock REST API server
    pub fn new(config: MockRestApiConfig) -> Self {
        let app_state = MockAppState {
            mock_data: Arc::new(MockData::new()),
        };

        Self { config, app_state }
    }

    /// Start the mock REST API server
    pub async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let app = self.create_router();
        let addr = format!("{}:{}", self.config.host, self.config.port);

        info!("Starting Mock REST API server on {}", addr);
        info!(
            "Serving {} mock DSL entries",
            self.app_state.mock_data.dsl_entries.len()
        );

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }

    /// Create the router with all endpoints
    fn create_router(&self) -> Router {
        Router::new()
            .route("/api/dsls", get(list_dsls))
            .route("/api/dsls/:id/ast", get(get_dsl_ast))
            .route("/api/health", get(health_check))
            .layer(
                ServiceBuilder::new().layer(CorsLayer::new().allow_origin(Any).allow_methods(Any)),
            )
            .with_state(self.app_state.clone())
    }
}

/// Health check endpoint
async fn health_check() -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    Ok(Json(serde_json::json!({
        "status": "healthy",
        "service": "mock-dsl-visualizer-api",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "mode": "mock"
    })))
}

/// List DSL instances with optional filtering
async fn list_dsls(
    State(state): State<MockAppState>,
    Query(params): Query<DslListQuery>,
) -> Result<Json<DslListResponse>, (StatusCode, String)> {
    info!(
        "Mock API: Listing DSLs - search: {:?}, domain: {:?}, limit: {:?}, offset: {:?}",
        params.search, params.domain, params.limit, params.offset
    );

    let limit = params.limit.unwrap_or(50).min(100) as usize;
    let offset = params.offset.unwrap_or(0).max(0) as usize;

    // Filter entries based on search and domain
    let mut filtered_entries: Vec<DslEntryResponse> = state
        .mock_data
        .dsl_entries
        .iter()
        .filter(|entry| {
            // Domain filter
            if let Some(ref domain_filter) = params.domain {
                if !domain_filter.is_empty() && entry.domain != *domain_filter {
                    return false;
                }
            }

            // Search filter
            if let Some(ref search_term) = params.search {
                if !search_term.is_empty() {
                    let search_lower = search_term.to_lowercase();
                    let matches_name = entry.name.to_lowercase().contains(&search_lower);
                    let matches_domain = entry.domain.to_lowercase().contains(&search_lower);
                    let matches_description =
                        entry.description.to_lowercase().contains(&search_lower);

                    if !(matches_name || matches_domain || matches_description) {
                        return false;
                    }
                }
            }

            true
        })
        .cloned()
        .collect();

    // Apply pagination
    let total_count = filtered_entries.len();
    if offset < filtered_entries.len() {
        let end = (offset + limit).min(filtered_entries.len());
        filtered_entries = filtered_entries[offset..end].to_vec();
    } else {
        filtered_entries.clear();
    }

    Ok(Json(DslListResponse {
        entries: filtered_entries,
        total_count: total_count as u32,
    }))
}

/// Get DSL content and AST for a specific instance
async fn get_dsl_ast(
    State(state): State<MockAppState>,
    Path(instance_id): Path<String>,
) -> Result<Json<DslContentResponse>, (StatusCode, String)> {
    info!("Mock API: Getting DSL AST for instance: {}", instance_id);

    match state.mock_data.dsl_contents.get(&instance_id) {
        Some(content) => Ok(Json(content.clone())),
        None => {
            warn!("Mock API: DSL instance {} not found", instance_id);
            Err((
                StatusCode::NOT_FOUND,
                format!("DSL instance {} not found", instance_id),
            ))
        }
    }
}

/// Create mock AST for Zenith Capital UBO
fn create_zenith_ast() -> AstNodeResponse {
    let mut properties = HashMap::new();
    properties.insert(
        "target-entity".to_string(),
        "company-zenith-spv-001".to_string(),
    );
    properties.insert("jurisdiction".to_string(), "KY".to_string());
    properties.insert("ubo-threshold".to_string(), "25.0".to_string());

    AstNodeResponse {
        id: "root-001".to_string(),
        node_type: "program".to_string(),
        label: "KYC Investigation".to_string(),
        properties,
        position: Some((400.0, 100.0)),
        children: vec![
            AstNodeResponse {
                id: "declare-001".to_string(),
                node_type: "verb".to_string(),
                label: "declare-entity".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("node-id".to_string(), "company-zenith-spv-001".to_string());
                    props.insert("label".to_string(), "Company".to_string());
                    props
                },
                position: Some((300.0, 200.0)),
                children: vec![],
            },
            AstNodeResponse {
                id: "edge-001".to_string(),
                node_type: "verb".to_string(),
                label: "create-edge".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("from".to_string(), "alpha-holdings-sg".to_string());
                    props.insert("to".to_string(), "company-zenith-spv-001".to_string());
                    props.insert("type".to_string(), "HAS_OWNERSHIP".to_string());
                    props.insert("percent".to_string(), "45.0".to_string());
                    props
                },
                position: Some((500.0, 200.0)),
                children: vec![],
            },
            AstNodeResponse {
                id: "validate-001".to_string(),
                node_type: "verb".to_string(),
                label: "validate".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert(
                        "attribute".to_string(),
                        "customer.email_primary".to_string(),
                    );
                    props.insert("value".to_string(), "john@zenithcapital.com".to_string());
                    props
                },
                position: Some((200.0, 300.0)),
                children: vec![],
            },
            AstNodeResponse {
                id: "collect-001".to_string(),
                node_type: "verb".to_string(),
                label: "collect".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("attribute".to_string(), "kyc.risk_rating".to_string());
                    props.insert("from".to_string(), "risk-engine".to_string());
                    props
                },
                position: Some((400.0, 300.0)),
                children: vec![],
            },
            AstNodeResponse {
                id: "check-001".to_string(),
                node_type: "verb".to_string(),
                label: "check".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert(
                        "attribute".to_string(),
                        "compliance.fatca_status".to_string(),
                    );
                    props.insert("equals".to_string(), "NON_US".to_string());
                    props
                },
                position: Some((600.0, 300.0)),
                children: vec![],
            },
        ],
    }
}

/// Create mock AST for UCITS Fund
fn create_ucits_ast() -> AstNodeResponse {
    AstNodeResponse {
        id: "root-002".to_string(),
        node_type: "program".to_string(),
        label: "UCITS Onboarding".to_string(),
        properties: HashMap::new(),
        position: Some((400.0, 100.0)),
        children: vec![
            AstNodeResponse {
                id: "case-001".to_string(),
                node_type: "verb".to_string(),
                label: "case.create".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("cbu-id".to_string(), "CBU-UCITS-001".to_string());
                    props.insert(
                        "nature-purpose".to_string(),
                        "UCITS equity fund domiciled in LU".to_string(),
                    );
                    props
                },
                position: Some((300.0, 200.0)),
                children: vec![],
            },
            AstNodeResponse {
                id: "products-001".to_string(),
                node_type: "verb".to_string(),
                label: "products.add".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert(
                        "services".to_string(),
                        "CUSTODY, FUND_ACCOUNTING, REGULATORY_REPORTING".to_string(),
                    );
                    props
                },
                position: Some((500.0, 200.0)),
                children: vec![],
            },
            AstNodeResponse {
                id: "services-001".to_string(),
                node_type: "verb".to_string(),
                label: "services.plan".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("settlement-sla".to_string(), "T+1".to_string());
                    props.insert("valuation-frequency".to_string(), "daily".to_string());
                    props
                },
                position: Some((400.0, 300.0)),
                children: vec![],
            },
        ],
    }
}

/// Create mock AST for KYC Risk Assessment
fn create_kyc_ast() -> AstNodeResponse {
    AstNodeResponse {
        id: "root-003".to_string(),
        node_type: "program".to_string(),
        label: "KYC Assessment".to_string(),
        properties: {
            let mut props = HashMap::new();
            props.insert("client-type".to_string(), "individual".to_string());
            props.insert("risk-category".to_string(), "high".to_string());
            props
        },
        position: Some((400.0, 100.0)),
        children: vec![
            AstNodeResponse {
                id: "collect-docs-001".to_string(),
                node_type: "verb".to_string(),
                label: "collect.documents".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("passport".to_string(), "verified".to_string());
                    props.insert("proof-of-address".to_string(), "pending".to_string());
                    props
                },
                position: Some((250.0, 200.0)),
                children: vec![],
            },
            AstNodeResponse {
                id: "screening-001".to_string(),
                node_type: "verb".to_string(),
                label: "screening.sanctions".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert(
                        "databases".to_string(),
                        "OFAC, EU_SANCTIONS, UN_SANCTIONS".to_string(),
                    );
                    props.insert("match-threshold".to_string(), "85.0".to_string());
                    props
                },
                position: Some((400.0, 200.0)),
                children: vec![],
            },
            AstNodeResponse {
                id: "pep-001".to_string(),
                node_type: "verb".to_string(),
                label: "screening.pep".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert(
                        "databases".to_string(),
                        "WORLD_CHECK, REFINITIV".to_string(),
                    );
                    props.insert("relationship-depth".to_string(), "2".to_string());
                    props
                },
                position: Some((550.0, 200.0)),
                children: vec![],
            },
            AstNodeResponse {
                id: "risk-scoring-001".to_string(),
                node_type: "verb".to_string(),
                label: "risk.scoring".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert(
                        "factors".to_string(),
                        "geography, occupation, transaction-patterns".to_string(),
                    );
                    props.insert("model".to_string(), "enhanced-v2.1".to_string());
                    props
                },
                position: Some((325.0, 300.0)),
                children: vec![],
            },
            AstNodeResponse {
                id: "approval-001".to_string(),
                node_type: "verb".to_string(),
                label: "approval.required".to_string(),
                properties: {
                    let mut props = HashMap::new();
                    props.insert("level".to_string(), "senior-compliance-officer".to_string());
                    props.insert("reason".to_string(), "high-risk-classification".to_string());
                    props
                },
                position: Some((475.0, 300.0)),
                children: vec![],
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_data_creation() {
        let mock_data = MockData::new();
        assert_eq!(mock_data.dsl_entries.len(), 5);
        assert_eq!(mock_data.dsl_contents.len(), 3);
    }

    #[test]
    fn test_zenith_ast_structure() {
        let ast = create_zenith_ast();
        assert_eq!(ast.node_type, "program");
        assert_eq!(ast.label, "KYC Investigation");
        assert_eq!(ast.children.len(), 5);

        // Check first child is declare-entity
        assert_eq!(ast.children[0].label, "declare-entity");
        assert_eq!(ast.children[0].node_type, "verb");
    }

    #[test]
    fn test_mock_config_default() {
        let config = MockRestApiConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);
    }
}
