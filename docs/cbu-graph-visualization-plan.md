# CBU Graph Visualization Implementation Plan

**Document**: `cbu-graph-visualization-plan.md`  
**Created**: 2025-12-01  
**Status**: READY FOR IMPLEMENTATION  
**Approach**: Rip and replace - remove existing UI, implement fresh

---

## Overview

Build a CBU visualization system with:
1. **CbuGraph** - Rust IR (intermediate representation) for graph data
2. **Graph Builder** - Loads CBU data from PostgreSQL, produces CbuGraph
3. **Axum API** - Endpoint serving CbuGraph as JSON
4. **egui WASM App** - Browser-based pan/zoom graph viewer
5. **Agent Panel** - Prompt input for agentic DSL generation

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Browser                                  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │                 egui WASM Application                      │  │
│  │  ┌─────────────────┐  ┌─────────────────────────────────┐ │  │
│  │  │  Agent Panel    │  │      Graph Canvas               │ │  │
│  │  │  ┌───────────┐  │  │  ┌───────────────────────────┐  │ │  │
│  │  │  │Domain: ▼  │  │  │  │                           │  │ │  │
│  │  │  └───────────┘  │  │  │    [CBU: Pacific Fund]    │  │ │  │
│  │  │  ┌───────────┐  │  │  │          │                │  │ │  │
│  │  │  │ Prompt    │  │  │  │    ┌─────┴─────┐          │  │ │  │
│  │  │  │           │  │  │  │  [SSI]     [Rules]        │  │ │  │
│  │  │  └───────────┘  │  │  │                           │  │ │  │
│  │  │  [Generate]     │  │  │  Pan: drag │ Zoom: scroll │  │ │  │
│  │  │                 │  │  └───────────────────────────┘  │ │  │
│  │  │  DSL Output:    │  │                                 │ │  │
│  │  │  ┌───────────┐  │  │  Layer toggles:                │ │  │
│  │  │  │ // dsl... │  │  │  [■ Custody] [□ KYC] [□ UBO]   │ │  │
│  │  │  └───────────┘  │  │                                 │ │  │
│  │  └─────────────────┘  └─────────────────────────────────┘ │  │
│  └───────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
                                │
                                │ HTTP/JSON
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Axum Server (Rust)                          │
│                                                                  │
│  GET  /api/cbu                     - List all CBUs              │
│  GET  /api/cbu/{id}                - CBU summary                │
│  GET  /api/cbu/{id}/graph          - CbuGraph for visualization │
│  POST /api/agent/custody/generate  - Agentic DSL generation     │
│  POST /api/agent/custody/plan      - Show plan only             │
│                                                                  │
│  Static: /app/*  → serves WASM app files                        │
└─────────────────────────────────────────────────────────────────┘
```

---

## Phase 1: CbuGraph Data Structure

### File: `rust/src/graph/mod.rs`

```rust
pub mod types;
pub mod builder;

pub use types::*;
pub use builder::CbuGraphBuilder;
```

### File: `rust/src/graph/types.rs`

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Graph projection of a CBU for visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CbuGraph {
    pub cbu_id: Uuid,
    pub label: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub layers: Vec<LayerInfo>,
    pub stats: GraphStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub node_type: NodeType,
    pub layer: LayerType,
    pub label: String,
    pub sublabel: Option<String>,
    pub status: NodeStatus,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    // Core
    Cbu,
    
    // Custody
    Universe,
    Ssi,
    BookingRule,
    Isda,
    Csa,
    Subcustodian,
    
    // KYC
    Document,
    Attribute,
    Verification,
    
    // UBO
    Entity,
    OwnershipLink,
    
    // Services
    Product,
    Service,
    Resource,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum LayerType {
    Core,
    Custody,
    Kyc,
    Ubo,
    Services,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeStatus {
    Active,
    Pending,
    Suspended,
    Expired,
    Draft,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: EdgeType,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeType {
    // Custody
    RoutesTo,
    Matches,
    CoveredBy,
    SecuredBy,
    SettlesAt,
    
    // KYC
    Requires,
    Validates,
    
    // UBO
    Owns,
    Controls,
    
    // Services
    Delivers,
    BelongsTo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerInfo {
    pub layer_type: LayerType,
    pub label: String,
    pub color: String,
    pub node_count: usize,
    pub visible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphStats {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub nodes_by_layer: HashMap<LayerType, usize>,
    pub nodes_by_type: HashMap<NodeType, usize>,
}

impl CbuGraph {
    pub fn new(cbu_id: Uuid, label: String) -> Self {
        Self {
            cbu_id,
            label,
            nodes: Vec::new(),
            edges: Vec::new(),
            layers: Vec::new(),
            stats: GraphStats::default(),
        }
    }
    
    pub fn add_node(&mut self, node: GraphNode) {
        self.nodes.push(node);
    }
    
    pub fn add_edge(&mut self, edge: GraphEdge) {
        self.edges.push(edge);
    }
    
    pub fn compute_stats(&mut self) {
        self.stats.total_nodes = self.nodes.len();
        self.stats.total_edges = self.edges.len();
        
        self.stats.nodes_by_layer.clear();
        self.stats.nodes_by_type.clear();
        
        for node in &self.nodes {
            *self.stats.nodes_by_layer.entry(node.layer).or_insert(0) += 1;
            *self.stats.nodes_by_type.entry(node.node_type).or_insert(0) += 1;
        }
    }
    
    pub fn build_layer_info(&mut self) {
        self.layers = vec![
            LayerInfo {
                layer_type: LayerType::Core,
                label: "Core".to_string(),
                color: "#6B7280".to_string(), // Gray
                node_count: self.stats.nodes_by_layer.get(&LayerType::Core).copied().unwrap_or(0),
                visible: true,
            },
            LayerInfo {
                layer_type: LayerType::Custody,
                label: "Custody".to_string(),
                color: "#3B82F6".to_string(), // Blue
                node_count: self.stats.nodes_by_layer.get(&LayerType::Custody).copied().unwrap_or(0),
                visible: true,
            },
            LayerInfo {
                layer_type: LayerType::Kyc,
                label: "KYC".to_string(),
                color: "#8B5CF6".to_string(), // Purple
                node_count: self.stats.nodes_by_layer.get(&LayerType::Kyc).copied().unwrap_or(0),
                visible: false,
            },
            LayerInfo {
                layer_type: LayerType::Ubo,
                label: "UBO".to_string(),
                color: "#10B981".to_string(), // Green
                node_count: self.stats.nodes_by_layer.get(&LayerType::Ubo).copied().unwrap_or(0),
                visible: false,
            },
            LayerInfo {
                layer_type: LayerType::Services,
                label: "Services".to_string(),
                color: "#F59E0B".to_string(), // Amber
                node_count: self.stats.nodes_by_layer.get(&LayerType::Services).copied().unwrap_or(0),
                visible: false,
            },
        ];
    }
}
```

### File: `rust/src/graph/builder.rs`

```rust
use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

use super::types::*;

pub struct CbuGraphBuilder {
    cbu_id: Uuid,
    include_custody: bool,
    include_kyc: bool,
    include_ubo: bool,
    include_services: bool,
}

impl CbuGraphBuilder {
    pub fn new(cbu_id: Uuid) -> Self {
        Self {
            cbu_id,
            include_custody: true,
            include_kyc: false,
            include_ubo: false,
            include_services: false,
        }
    }
    
    pub fn with_custody(mut self, include: bool) -> Self {
        self.include_custody = include;
        self
    }
    
    pub fn with_kyc(mut self, include: bool) -> Self {
        self.include_kyc = include;
        self
    }
    
    pub fn with_ubo(mut self, include: bool) -> Self {
        self.include_ubo = include;
        self
    }
    
    pub fn with_services(mut self, include: bool) -> Self {
        self.include_services = include;
        self
    }
    
    pub async fn build(self, pool: &PgPool) -> Result<CbuGraph> {
        // Load CBU base record
        let cbu_record = sqlx::query!(
            r#"SELECT cbu_id, name, jurisdiction, client_type, status 
               FROM "ob-poc".cbus WHERE cbu_id = $1"#,
            self.cbu_id
        )
        .fetch_one(pool)
        .await?;
        
        let mut graph = CbuGraph::new(self.cbu_id, cbu_record.name.clone());
        
        // Add CBU root node
        graph.add_node(GraphNode {
            id: self.cbu_id.to_string(),
            node_type: NodeType::Cbu,
            layer: LayerType::Core,
            label: cbu_record.name,
            sublabel: Some(format!("{} / {}", cbu_record.jurisdiction, cbu_record.client_type)),
            status: match cbu_record.status.as_str() {
                "ACTIVE" => NodeStatus::Active,
                "PENDING" => NodeStatus::Pending,
                "SUSPENDED" => NodeStatus::Suspended,
                _ => NodeStatus::Draft,
            },
            data: serde_json::json!({
                "jurisdiction": cbu_record.jurisdiction,
                "client_type": cbu_record.client_type,
                "status": cbu_record.status
            }),
        });
        
        // Load custody layer
        if self.include_custody {
            self.load_custody_layer(&mut graph, pool).await?;
        }
        
        // Load KYC layer
        if self.include_kyc {
            self.load_kyc_layer(&mut graph, pool).await?;
        }
        
        // Load UBO layer
        if self.include_ubo {
            self.load_ubo_layer(&mut graph, pool).await?;
        }
        
        // Load Services layer
        if self.include_services {
            self.load_services_layer(&mut graph, pool).await?;
        }
        
        // Compute final stats
        graph.compute_stats();
        graph.build_layer_info();
        
        Ok(graph)
    }
    
    async fn load_custody_layer(&self, graph: &mut CbuGraph, pool: &PgPool) -> Result<()> {
        // Load universe entries
        let universes = sqlx::query!(
            r#"SELECT universe_id, instrument_class_id, market_id, currencies, 
                      settlement_types, is_active
               FROM custody.cbu_instrument_universe 
               WHERE cbu_id = $1"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await?;
        
        for u in universes {
            let universe_id = u.universe_id.to_string();
            
            // Get display names via joins (simplified - might need actual join query)
            let label = format!("Universe {}", &universe_id[..8]);
            
            graph.add_node(GraphNode {
                id: universe_id.clone(),
                node_type: NodeType::Universe,
                layer: LayerType::Custody,
                label,
                sublabel: u.currencies.as_ref().map(|c| c.join(", ")),
                status: if u.is_active.unwrap_or(true) { 
                    NodeStatus::Active 
                } else { 
                    NodeStatus::Suspended 
                },
                data: serde_json::json!({
                    "instrument_class_id": u.instrument_class_id,
                    "market_id": u.market_id,
                    "currencies": u.currencies,
                    "settlement_types": u.settlement_types
                }),
            });
            
            // Edge: CBU → Universe
            graph.add_edge(GraphEdge {
                id: format!("cbu->{}", universe_id),
                source: self.cbu_id.to_string(),
                target: universe_id,
                edge_type: EdgeType::Matches,
                label: None,
            });
        }
        
        // Load SSIs
        let ssis = sqlx::query!(
            r#"SELECT ssi_id, ssi_name, ssi_type, status, cash_currency,
                      safekeeping_account, cash_account, subcustodian_bic
               FROM custody.cbu_ssi 
               WHERE cbu_id = $1"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await?;
        
        for ssi in ssis {
            let ssi_id = ssi.ssi_id.to_string();
            
            graph.add_node(GraphNode {
                id: ssi_id.clone(),
                node_type: NodeType::Ssi,
                layer: LayerType::Custody,
                label: ssi.ssi_name,
                sublabel: Some(format!("{} @ {}", 
                    ssi.cash_currency.unwrap_or_default(),
                    ssi.subcustodian_bic.as_deref().unwrap_or("N/A")
                )),
                status: match ssi.status.as_deref().unwrap_or("DRAFT") {
                    "ACTIVE" => NodeStatus::Active,
                    "PENDING" => NodeStatus::Pending,
                    "SUSPENDED" => NodeStatus::Suspended,
                    _ => NodeStatus::Draft,
                },
                data: serde_json::json!({
                    "ssi_type": ssi.ssi_type,
                    "cash_currency": ssi.cash_currency,
                    "safekeeping_account": ssi.safekeeping_account,
                    "cash_account": ssi.cash_account,
                    "subcustodian_bic": ssi.subcustodian_bic
                }),
            });
            
            // Edge: CBU → SSI
            graph.add_edge(GraphEdge {
                id: format!("cbu->{}", ssi_id),
                source: self.cbu_id.to_string(),
                target: ssi_id,
                edge_type: EdgeType::SettlesAt,
                label: None,
            });
        }
        
        // Load booking rules
        let rules = sqlx::query!(
            r#"SELECT rule_id, rule_name, priority, ssi_id, 
                      instrument_class_id, market_id, currency, is_active
               FROM custody.ssi_booking_rules 
               WHERE cbu_id = $1
               ORDER BY priority"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await?;
        
        for rule in rules {
            let rule_id = rule.rule_id.to_string();
            
            graph.add_node(GraphNode {
                id: rule_id.clone(),
                node_type: NodeType::BookingRule,
                layer: LayerType::Custody,
                label: rule.rule_name,
                sublabel: Some(format!("Priority {}", rule.priority)),
                status: if rule.is_active.unwrap_or(true) { 
                    NodeStatus::Active 
                } else { 
                    NodeStatus::Suspended 
                },
                data: serde_json::json!({
                    "priority": rule.priority,
                    "instrument_class_id": rule.instrument_class_id,
                    "market_id": rule.market_id,
                    "currency": rule.currency
                }),
            });
            
            // Edge: Rule → SSI (routes to)
            graph.add_edge(GraphEdge {
                id: format!("{}->{}", rule_id, rule.ssi_id),
                source: rule_id,
                target: rule.ssi_id.to_string(),
                edge_type: EdgeType::RoutesTo,
                label: None,
            });
        }
        
        // Load ISDA agreements
        let isdas = sqlx::query!(
            r#"SELECT isda_id, counterparty_entity_id, governing_law, 
                      agreement_date, status
               FROM custody.isda_agreements 
               WHERE cbu_id = $1"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await?;
        
        for isda in isdas {
            let isda_id = isda.isda_id.to_string();
            
            graph.add_node(GraphNode {
                id: isda_id.clone(),
                node_type: NodeType::Isda,
                layer: LayerType::Custody,
                label: format!("ISDA ({})", isda.governing_law.as_deref().unwrap_or("N/A")),
                sublabel: isda.agreement_date.map(|d| d.to_string()),
                status: match isda.status.as_deref().unwrap_or("DRAFT") {
                    "ACTIVE" => NodeStatus::Active,
                    "PENDING" => NodeStatus::Pending,
                    _ => NodeStatus::Draft,
                },
                data: serde_json::json!({
                    "counterparty_entity_id": isda.counterparty_entity_id,
                    "governing_law": isda.governing_law,
                    "agreement_date": isda.agreement_date
                }),
            });
            
            // Edge: CBU → ISDA
            graph.add_edge(GraphEdge {
                id: format!("cbu->{}", isda_id),
                source: self.cbu_id.to_string(),
                target: isda_id.clone(),
                edge_type: EdgeType::CoveredBy,
                label: None,
            });
            
            // Load CSAs for this ISDA
            let csas = sqlx::query!(
                r#"SELECT csa_id, csa_type, margin_type, eligible_collateral
                   FROM custody.csa_agreements 
                   WHERE isda_id = $1"#,
                isda.isda_id
            )
            .fetch_all(pool)
            .await?;
            
            for csa in csas {
                let csa_id = csa.csa_id.to_string();
                
                graph.add_node(GraphNode {
                    id: csa_id.clone(),
                    node_type: NodeType::Csa,
                    layer: LayerType::Custody,
                    label: format!("CSA ({})", csa.margin_type.as_deref().unwrap_or("VM")),
                    sublabel: csa.csa_type.clone(),
                    status: NodeStatus::Active,
                    data: serde_json::json!({
                        "csa_type": csa.csa_type,
                        "margin_type": csa.margin_type,
                        "eligible_collateral": csa.eligible_collateral
                    }),
                });
                
                // Edge: ISDA → CSA
                graph.add_edge(GraphEdge {
                    id: format!("{}->{}", isda_id, csa_id),
                    source: isda_id.clone(),
                    target: csa_id,
                    edge_type: EdgeType::SecuredBy,
                    label: None,
                });
            }
        }
        
        Ok(())
    }
    
    async fn load_kyc_layer(&self, graph: &mut CbuGraph, pool: &PgPool) -> Result<()> {
        // Load documents linked to CBU's entity
        // This depends on your KYC schema structure
        // Placeholder implementation
        
        let docs = sqlx::query!(
            r#"SELECT d.document_id, d.document_type, d.status, d.file_name
               FROM "ob-poc".documents d
               JOIN "ob-poc".cbus c ON c.entity_id = d.entity_id
               WHERE c.cbu_id = $1"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await;
        
        if let Ok(docs) = docs {
            for doc in docs {
                let doc_id = doc.document_id.to_string();
                
                graph.add_node(GraphNode {
                    id: doc_id.clone(),
                    node_type: NodeType::Document,
                    layer: LayerType::Kyc,
                    label: doc.document_type.unwrap_or_else(|| "Document".to_string()),
                    sublabel: doc.file_name,
                    status: match doc.status.as_deref().unwrap_or("PENDING") {
                        "VERIFIED" => NodeStatus::Active,
                        "REJECTED" => NodeStatus::Expired,
                        _ => NodeStatus::Pending,
                    },
                    data: serde_json::json!({}),
                });
                
                graph.add_edge(GraphEdge {
                    id: format!("cbu->{}", doc_id),
                    source: self.cbu_id.to_string(),
                    target: doc_id,
                    edge_type: EdgeType::Requires,
                    label: None,
                });
            }
        }
        
        Ok(())
    }
    
    async fn load_ubo_layer(&self, graph: &mut CbuGraph, pool: &PgPool) -> Result<()> {
        // Load ownership chain from entity_ownership_links
        // This is recursive - might need CTE query
        // Placeholder implementation
        
        let links = sqlx::query!(
            r#"SELECT eol.link_id, eol.parent_entity_id, eol.child_entity_id,
                      eol.ownership_percentage, eol.ownership_type,
                      pe.legal_name as parent_name,
                      ce.legal_name as child_name
               FROM "ob-poc".entity_ownership_links eol
               JOIN "ob-poc".entities pe ON pe.entity_id = eol.parent_entity_id
               JOIN "ob-poc".entities ce ON ce.entity_id = eol.child_entity_id
               JOIN "ob-poc".cbus c ON c.entity_id = eol.child_entity_id
               WHERE c.cbu_id = $1"#,
            self.cbu_id
        )
        .fetch_all(pool)
        .await;
        
        if let Ok(links) = links {
            let mut seen_entities = std::collections::HashSet::new();
            
            for link in links {
                // Add parent entity if not seen
                let parent_id = link.parent_entity_id.to_string();
                if seen_entities.insert(parent_id.clone()) {
                    graph.add_node(GraphNode {
                        id: parent_id.clone(),
                        node_type: NodeType::Entity,
                        layer: LayerType::Ubo,
                        label: link.parent_name.unwrap_or_else(|| "Entity".to_string()),
                        sublabel: None,
                        status: NodeStatus::Active,
                        data: serde_json::json!({}),
                    });
                }
                
                // Add child entity if not seen
                let child_id = link.child_entity_id.to_string();
                if seen_entities.insert(child_id.clone()) {
                    graph.add_node(GraphNode {
                        id: child_id.clone(),
                        node_type: NodeType::Entity,
                        layer: LayerType::Ubo,
                        label: link.child_name.unwrap_or_else(|| "Entity".to_string()),
                        sublabel: None,
                        status: NodeStatus::Active,
                        data: serde_json::json!({}),
                    });
                }
                
                // Add ownership edge
                let pct = link.ownership_percentage
                    .map(|p| format!("{}%", p))
                    .unwrap_or_default();
                    
                graph.add_edge(GraphEdge {
                    id: link.link_id.to_string(),
                    source: parent_id,
                    target: child_id,
                    edge_type: EdgeType::Owns,
                    label: Some(pct),
                });
            }
        }
        
        Ok(())
    }
    
    async fn load_services_layer(&self, graph: &mut CbuGraph, pool: &PgPool) -> Result<()> {
        // Load from product_service_delivery or similar
        // Placeholder - depends on your service delivery schema
        Ok(())
    }
}
```

**Effort**: 1 day

---

## Phase 2: Axum API Endpoints

### File: `rust/src/api/graph_routes.rs`

```rust
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::graph::{CbuGraph, CbuGraphBuilder};
use crate::AppState;

#[derive(Deserialize)]
pub struct GraphQueryParams {
    #[serde(default = "default_true")]
    pub custody: bool,
    #[serde(default)]
    pub kyc: bool,
    #[serde(default)]
    pub ubo: bool,
    #[serde(default)]
    pub services: bool,
}

fn default_true() -> bool { true }

/// GET /api/cbu/{cbu_id}/graph
pub async fn get_cbu_graph(
    State(state): State<AppState>,
    Path(cbu_id): Path<Uuid>,
    Query(params): Query<GraphQueryParams>,
) -> Result<Json<CbuGraph>, (StatusCode, String)> {
    let graph = CbuGraphBuilder::new(cbu_id)
        .with_custody(params.custody)
        .with_kyc(params.kyc)
        .with_ubo(params.ubo)
        .with_services(params.services)
        .build(&state.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    Ok(Json(graph))
}

/// GET /api/cbu - List all CBUs (summary)
pub async fn list_cbus(
    State(state): State<AppState>,
) -> Result<Json<Vec<CbuSummary>>, (StatusCode, String)> {
    let cbus = sqlx::query_as!(
        CbuSummary,
        r#"SELECT cbu_id, name, jurisdiction, client_type, status,
                  created_at, updated_at
           FROM "ob-poc".cbus
           ORDER BY name"#
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    
    Ok(Json(cbus))
}

#[derive(serde::Serialize, sqlx::FromRow)]
pub struct CbuSummary {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: String,
    pub client_type: String,
    pub status: String,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}
```

### Update: `rust/src/api/mod.rs`

Add graph routes to router:

```rust
pub mod graph_routes;

// In router setup:
.route("/api/cbu", get(graph_routes::list_cbus))
.route("/api/cbu/:cbu_id/graph", get(graph_routes::get_cbu_graph))
```

**Effort**: 0.5 day

---

## Phase 3: egui WASM Application

### New Crate: `rust/crates/ob-poc-ui/`

Create a new crate for the WASM UI:

```
rust/crates/ob-poc-ui/
├── Cargo.toml
├── src/
│   ├── main.rs          # Entry point
│   ├── app.rs           # Main application state
│   ├── api.rs           # HTTP client for backend
│   ├── graph_view.rs    # Pan/zoom graph canvas
│   ├── agent_panel.rs   # Prompt input panel
│   ├── node_styles.rs   # Visual styling per node type
│   └── layout.rs        # Graph layout algorithm
└── index.html           # HTML shell
```

### File: `rust/crates/ob-poc-ui/Cargo.toml`

```toml
[package]
name = "ob-poc-ui"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
eframe = { version = "0.28", default-features = false, features = [
    "accesskit",
    "default_fonts",
    "glow",
    "wasm_executor"
] }
egui = "0.28"
egui_extras = "0.28"

# Async HTTP
reqwest = { version = "0.12", features = ["json"] }
tokio = { version = "1", features = ["rt"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Shared types (if extracted to shared crate)
# ob-poc-graph-types = { path = "../ob-poc-graph-types" }

# WASM support
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
console_error_panic_hook = "0.1"
tracing-wasm = "0.2"

[target.'cfg(target_arch = "wasm32")'.dependencies]
web-sys = { version = "0.3", features = ["console"] }
```

### File: `rust/crates/ob-poc-ui/src/main.rs`

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod api;
mod graph_view;
mod agent_panel;
mod node_styles;
mod layout;

use app::ObPocApp;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    tracing_subscriber::fmt::init();
    
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    
    eframe::run_native(
        "OB-POC Visualization",
        native_options,
        Box::new(|cc| Ok(Box::new(ObPocApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    
    let web_options = eframe::WebOptions::default();
    
    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                "ob_poc_canvas",
                web_options,
                Box::new(|cc| Ok(Box::new(ObPocApp::new(cc)))),
            )
            .await
            .expect("Failed to start eframe");
    });
}
```

### File: `rust/crates/ob-poc-ui/src/app.rs`

```rust
use eframe::egui;
use crate::api::ApiClient;
use crate::graph_view::GraphView;
use crate::agent_panel::AgentPanel;

pub struct ObPocApp {
    api: ApiClient,
    graph_view: GraphView,
    agent_panel: AgentPanel,
    selected_cbu: Option<uuid::Uuid>,
    cbu_list: Vec<CbuSummary>,
    loading: bool,
    error: Option<String>,
}

#[derive(Clone, serde::Deserialize)]
pub struct CbuSummary {
    pub cbu_id: uuid::Uuid,
    pub name: String,
    pub jurisdiction: String,
    pub client_type: String,
    pub status: String,
}

impl ObPocApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            api: ApiClient::new("http://localhost:8080"),
            graph_view: GraphView::new(),
            agent_panel: AgentPanel::new(),
            selected_cbu: None,
            cbu_list: Vec::new(),
            loading: false,
            error: None,
        }
    }
    
    fn load_cbu_list(&mut self, ctx: &egui::Context) {
        self.loading = true;
        let api = self.api.clone();
        let ctx = ctx.clone();
        
        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            // Fetch and update via channel or shared state
        });
    }
    
    fn load_cbu_graph(&mut self, cbu_id: uuid::Uuid, ctx: &egui::Context) {
        self.loading = true;
        // Similar async load
    }
}

impl eframe::App for ObPocApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top panel - CBU selector
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("CBU:");
                
                egui::ComboBox::from_id_salt("cbu_selector")
                    .selected_text(
                        self.selected_cbu
                            .and_then(|id| {
                                self.cbu_list.iter().find(|c| c.cbu_id == id)
                            })
                            .map(|c| c.name.as_str())
                            .unwrap_or("Select CBU...")
                    )
                    .show_ui(ui, |ui| {
                        for cbu in &self.cbu_list {
                            if ui.selectable_label(
                                self.selected_cbu == Some(cbu.cbu_id),
                                &cbu.name
                            ).clicked() {
                                self.selected_cbu = Some(cbu.cbu_id);
                                self.load_cbu_graph(cbu.cbu_id, ctx);
                            }
                        }
                    });
                
                if ui.button("🔄 Refresh").clicked() {
                    self.load_cbu_list(ctx);
                }
                
                ui.separator();
                
                // Layer toggles
                ui.label("Layers:");
                ui.checkbox(&mut self.graph_view.show_custody, "Custody");
                ui.checkbox(&mut self.graph_view.show_kyc, "KYC");
                ui.checkbox(&mut self.graph_view.show_ubo, "UBO");
                ui.checkbox(&mut self.graph_view.show_services, "Services");
            });
        });
        
        // Left panel - Agent prompt
        egui::SidePanel::left("agent_panel")
            .default_width(350.0)
            .show(ctx, |ui| {
                self.agent_panel.ui(ui, &self.api);
            });
        
        // Central panel - Graph view
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.loading {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                });
            } else if let Some(ref err) = self.error {
                ui.colored_label(egui::Color32::RED, err);
            } else {
                self.graph_view.ui(ui);
            }
        });
    }
}
```

### File: `rust/crates/ob-poc-ui/src/graph_view.rs`

```rust
use egui::{Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::collections::HashMap;

// Import CbuGraph types (ideally from shared crate)
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Deserialize)]
pub struct CbuGraph {
    pub cbu_id: uuid::Uuid,
    pub label: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub layers: Vec<LayerInfo>,
}

#[derive(Clone, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub node_type: String,
    pub layer: String,
    pub label: String,
    pub sublabel: Option<String>,
    pub status: String,
    pub data: serde_json::Value,
}

#[derive(Clone, Deserialize)]
pub struct GraphEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub edge_type: String,
    pub label: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct LayerInfo {
    pub layer_type: String,
    pub label: String,
    pub color: String,
    pub node_count: usize,
    pub visible: bool,
}

pub struct GraphView {
    pub graph: Option<CbuGraph>,
    pub show_custody: bool,
    pub show_kyc: bool,
    pub show_ubo: bool,
    pub show_services: bool,
    
    // Pan/zoom state
    offset: Vec2,
    zoom: f32,
    
    // Layout cache
    node_positions: HashMap<String, Pos2>,
    selected_node: Option<String>,
}

impl GraphView {
    pub fn new() -> Self {
        Self {
            graph: None,
            show_custody: true,
            show_kyc: false,
            show_ubo: false,
            show_services: false,
            offset: Vec2::ZERO,
            zoom: 1.0,
            node_positions: HashMap::new(),
            selected_node: None,
        }
    }
    
    pub fn set_graph(&mut self, graph: CbuGraph) {
        self.graph = Some(graph);
        self.compute_layout();
    }
    
    fn compute_layout(&mut self) {
        let Some(ref graph) = self.graph else { return };
        
        self.node_positions.clear();
        
        // Simple hierarchical layout
        // Layer 0: CBU (center top)
        // Layer 1: Universe, SSI, ISDA
        // Layer 2: Rules, CSA
        // etc.
        
        let mut layer_counts: HashMap<&str, usize> = HashMap::new();
        let mut layer_positions: HashMap<&str, usize> = HashMap::new();
        
        // Count nodes per layer
        for node in &graph.nodes {
            *layer_counts.entry(node.layer.as_str()).or_insert(0) += 1;
        }
        
        // Assign positions
        let layer_y = |layer: &str| -> f32 {
            match layer {
                "core" => 50.0,
                "custody" => 200.0,
                "kyc" => 350.0,
                "ubo" => 500.0,
                "services" => 650.0,
                _ => 400.0,
            }
        };
        
        for node in &graph.nodes {
            let layer = node.layer.as_str();
            let count = layer_counts.get(layer).copied().unwrap_or(1);
            let pos = layer_positions.entry(layer).or_insert(0);
            
            let spacing = 180.0;
            let start_x = -(count as f32 - 1.0) * spacing / 2.0;
            let x = start_x + (*pos as f32) * spacing;
            let y = layer_y(layer);
            
            self.node_positions.insert(node.id.clone(), Pos2::new(x, y));
            *pos += 1;
        }
    }
    
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(ref graph) = self.graph else {
            ui.centered_and_justified(|ui| {
                ui.label("Select a CBU to visualize");
            });
            return;
        };
        
        // Get available rect
        let (response, painter) = ui.allocate_painter(
            ui.available_size(),
            Sense::click_and_drag()
        );
        
        let rect = response.rect;
        let center = rect.center();
        
        // Handle pan
        if response.dragged() {
            self.offset += response.drag_delta();
        }
        
        // Handle zoom (scroll)
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if scroll != 0.0 {
            let zoom_factor = 1.0 + scroll * 0.001;
            self.zoom = (self.zoom * zoom_factor).clamp(0.2, 3.0);
        }
        
        // Transform helper
        let transform = |pos: Pos2| -> Pos2 {
            center + (pos.to_vec2() + self.offset) * self.zoom
        };
        
        // Draw edges first (below nodes)
        for edge in &graph.edges {
            let Some(source_pos) = self.node_positions.get(&edge.source) else { continue };
            let Some(target_pos) = self.node_positions.get(&edge.target) else { continue };
            
            let from = transform(*source_pos);
            let to = transform(*target_pos);
            
            let color = Color32::from_rgb(150, 150, 150);
            painter.line_segment([from, to], Stroke::new(1.5 * self.zoom, color));
            
            // Optional: draw edge label
            if let Some(ref label) = edge.label {
                let mid = Pos2::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0);
                painter.text(
                    mid,
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(10.0 * self.zoom),
                    Color32::GRAY,
                );
            }
        }
        
        // Draw nodes
        for node in &graph.nodes {
            // Filter by layer visibility
            let visible = match node.layer.as_str() {
                "core" => true,
                "custody" => self.show_custody,
                "kyc" => self.show_kyc,
                "ubo" => self.show_ubo,
                "services" => self.show_services,
                _ => true,
            };
            
            if !visible { continue }
            
            let Some(pos) = self.node_positions.get(&node.id) else { continue };
            let screen_pos = transform(*pos);
            
            // Node styling based on type
            let (bg_color, border_color) = node_colors(&node.node_type, &node.status);
            
            let node_size = Vec2::new(140.0, 50.0) * self.zoom;
            let node_rect = Rect::from_center_size(screen_pos, node_size);
            
            // Draw node background
            painter.rect_filled(node_rect, 8.0 * self.zoom, bg_color);
            painter.rect_stroke(node_rect, 8.0 * self.zoom, Stroke::new(2.0 * self.zoom, border_color));
            
            // Draw label
            painter.text(
                screen_pos - Vec2::new(0.0, 8.0 * self.zoom),
                egui::Align2::CENTER_CENTER,
                &node.label,
                egui::FontId::proportional(12.0 * self.zoom),
                Color32::WHITE,
            );
            
            // Draw sublabel
            if let Some(ref sublabel) = node.sublabel {
                painter.text(
                    screen_pos + Vec2::new(0.0, 10.0 * self.zoom),
                    egui::Align2::CENTER_CENTER,
                    sublabel,
                    egui::FontId::proportional(10.0 * self.zoom),
                    Color32::from_rgb(200, 200, 200),
                );
            }
            
            // Handle click
            if response.clicked() {
                if let Some(pointer_pos) = response.interact_pointer_pos() {
                    if node_rect.contains(pointer_pos) {
                        self.selected_node = Some(node.id.clone());
                    }
                }
            }
        }
        
        // Draw zoom/pan info
        painter.text(
            rect.left_bottom() + Vec2::new(10.0, -10.0),
            egui::Align2::LEFT_BOTTOM,
            format!("Zoom: {:.0}%  |  Drag to pan, scroll to zoom", self.zoom * 100.0),
            egui::FontId::proportional(11.0),
            Color32::GRAY,
        );
    }
}

fn node_colors(node_type: &str, status: &str) -> (Color32, Color32) {
    let base = match node_type {
        "cbu" => Color32::from_rgb(75, 85, 99),        // Gray
        "universe" => Color32::from_rgb(59, 130, 246), // Blue
        "ssi" => Color32::from_rgb(16, 185, 129),      // Green
        "booking_rule" => Color32::from_rgb(245, 158, 11), // Amber
        "isda" => Color32::from_rgb(139, 92, 246),     // Purple
        "csa" => Color32::from_rgb(236, 72, 153),      // Pink
        "entity" => Color32::from_rgb(34, 197, 94),    // Emerald
        "document" => Color32::from_rgb(99, 102, 241), // Indigo
        _ => Color32::from_rgb(107, 114, 128),         // Gray
    };
    
    let border = match status {
        "active" => Color32::from_rgb(34, 197, 94),    // Green
        "pending" => Color32::from_rgb(251, 191, 36),  // Yellow
        "suspended" => Color32::from_rgb(239, 68, 68), // Red
        "expired" => Color32::from_rgb(107, 114, 128), // Gray
        _ => Color32::WHITE,
    };
    
    (base, border)
}
```

### File: `rust/crates/ob-poc-ui/src/agent_panel.rs`

```rust
use egui::{TextEdit, RichText};

pub struct AgentPanel {
    domain: AgentDomain,
    prompt: String,
    generated_dsl: Option<String>,
    plan_only: bool,
    loading: bool,
    error: Option<String>,
}

#[derive(Default, Clone, Copy, PartialEq)]
pub enum AgentDomain {
    #[default]
    Custody,
    Kyc,
    Ubo,
}

impl AgentPanel {
    pub fn new() -> Self {
        Self {
            domain: AgentDomain::Custody,
            prompt: String::new(),
            generated_dsl: None,
            plan_only: false,
            loading: false,
            error: None,
        }
    }
    
    pub fn ui(&mut self, ui: &mut egui::Ui, api: &crate::api::ApiClient) {
        ui.heading("Agent DSL Generator");
        ui.separator();
        
        // Domain selector
        ui.horizontal(|ui| {
            ui.label("Domain:");
            ui.selectable_value(&mut self.domain, AgentDomain::Custody, "Custody");
            ui.selectable_value(&mut self.domain, AgentDomain::Kyc, "KYC");
            ui.selectable_value(&mut self.domain, AgentDomain::Ubo, "UBO");
        });
        
        ui.add_space(8.0);
        
        // Prompt input
        ui.label("Describe what you want to set up:");
        ui.add(
            TextEdit::multiline(&mut self.prompt)
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .hint_text("e.g., Onboard Pacific Fund for US and UK equities with USD cross-currency...")
        );
        
        ui.add_space(8.0);
        
        // Options
        ui.checkbox(&mut self.plan_only, "Show plan only (don't generate DSL)");
        
        ui.add_space(8.0);
        
        // Generate button
        ui.horizontal(|ui| {
            let button_text = if self.loading { "Generating..." } else { "Generate DSL" };
            if ui.button(button_text).clicked() && !self.loading && !self.prompt.is_empty() {
                self.generate(api);
            }
            
            if ui.button("Clear").clicked() {
                self.prompt.clear();
                self.generated_dsl = None;
                self.error = None;
            }
        });
        
        ui.separator();
        
        // Output
        if let Some(ref err) = self.error {
            ui.colored_label(egui::Color32::RED, err);
        }
        
        if let Some(ref dsl) = self.generated_dsl {
            ui.label(RichText::new("Generated DSL:").strong());
            
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut dsl.as_str())
                            .code_editor()
                            .desired_width(f32::INFINITY)
                    );
                });
            
            ui.add_space(8.0);
            
            if ui.button("📋 Copy to Clipboard").clicked() {
                ui.output_mut(|o| o.copied_text = dsl.clone());
            }
        }
    }
    
    fn generate(&mut self, _api: &crate::api::ApiClient) {
        self.loading = true;
        self.error = None;
        
        // TODO: Async call to /api/agent/custody/generate
        // For now, placeholder
        
        #[cfg(target_arch = "wasm32")]
        {
            // wasm_bindgen_futures::spawn_local(...)
        }
    }
}
```

### File: `rust/crates/ob-poc-ui/src/api.rs`

```rust
use serde::de::DeserializeOwned;

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
        
        let response = reqwest::get(&url)
            .await
            .map_err(|e| e.to_string())?;
        
        if !response.status().is_success() {
            return Err(format!("HTTP {}", response.status()));
        }
        
        response
            .json::<T>()
            .await
            .map_err(|e| e.to_string())
    }
    
    pub async fn post<T: DeserializeOwned, B: serde::Serialize>(
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
        
        response
            .json::<T>()
            .await
            .map_err(|e| e.to_string())
    }
}
```

### File: `rust/crates/ob-poc-ui/index.html`

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OB-POC Visualization</title>
    <style>
        html, body {
            margin: 0;
            padding: 0;
            width: 100%;
            height: 100%;
            overflow: hidden;
            background: #1a1a2e;
        }
        
        #ob_poc_canvas {
            width: 100%;
            height: 100%;
        }
    </style>
</head>
<body>
    <canvas id="ob_poc_canvas"></canvas>
    <script type="module">
        import init from './pkg/ob_poc_ui.js';
        init();
    </script>
</body>
</html>
```

**Effort**: 2-3 days

---

## Phase 4: Build & Serve WASM

### File: `rust/crates/ob-poc-ui/build.sh`

```bash
#!/bin/bash
set -e

echo "Building WASM..."
wasm-pack build --target web --out-dir pkg

echo "Done! Serve with: python -m http.server 8081"
```

### Update Axum to Serve Static Files

In your main Axum app, add static file serving:

```rust
use tower_http::services::ServeDir;

// In router setup:
.nest_service("/app", ServeDir::new("crates/ob-poc-ui/pkg"))
.nest_service("/", ServeDir::new("crates/ob-poc-ui").append_index_html_on_directories(true))
```

### Build Commands

```bash
# Build WASM
cd rust/crates/ob-poc-ui
wasm-pack build --target web --out-dir pkg

# Run server
cd rust
cargo run --bin ob-poc-server

# Access at: http://localhost:8080/
```

**Effort**: 0.5 day

---

## Phase 5: Remove Existing UI

### What to Remove

```bash
# Remove existing UI code (specify paths based on current structure)
rm -rf rust/src/ui/           # If exists
rm -rf web/                   # If exists
rm -rf frontend/              # If exists

# Keep: rust/src/api/ (Axum routes)
```

### What to Keep

- All Axum API routes
- All DSL/agentic code
- Database layer
- CLI tools

**Effort**: 0.5 day (mostly cleanup)

---

## Summary

| Phase | Description | Effort |
|-------|-------------|--------|
| 1 | CbuGraph types + builder | 1 day |
| 2 | Axum API endpoints | 0.5 day |
| 3 | egui WASM application | 2-3 days |
| 4 | Build & serve setup | 0.5 day |
| 5 | Remove existing UI | 0.5 day |
| **Total** | | **4.5-5.5 days** |

---

## Testing Checklist

- [ ] `GET /api/cbu` returns list of CBUs
- [ ] `GET /api/cbu/{id}/graph` returns CbuGraph JSON
- [ ] `GET /api/cbu/{id}/graph?custody=true&ubo=true` returns combined layers
- [ ] WASM builds without errors
- [ ] Pan (drag) works in graph view
- [ ] Zoom (scroll) works in graph view
- [ ] Layer toggles filter nodes
- [ ] Agent panel sends request and displays DSL
- [ ] Node click shows selection

---

## Notes for Implementation

1. **Type Sharing**: Consider extracting CbuGraph types to a shared crate used by both server and UI to avoid duplication

2. **Layout Algorithm**: The simple hierarchical layout can be improved later with force-directed or Sugiyama algorithms

3. **Async in WASM**: egui with eframe handles async awkwardly - may need channels or `poll_promise` crate

4. **CORS**: Ensure Axum has CORS middleware enabled for WASM requests

5. **Error Handling**: Add proper error states and loading indicators throughout

---

*End of Implementation Plan*
