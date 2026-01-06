# TODO: Galaxy View Server API (Phase 1)

## Priority: IMMEDIATE - Blocks all client work

## Reference Documents

- **Node/Edge Taxonomy:** `GALAXY-NODE-EDGE-TAXONOMY.md` - Defines all node types, edge types, response structures, and rendering rules per LOD
- **Full Research:** `brain-dump/GALAXY-VIEW-RESEARCH.md` - Analysis of existing infrastructure
- **Master TODO:** `TODO-GALAXY-VIEW-WIRING.md` - Full implementation plan

---

## Endpoints Required

### 1. GET /api/universe

**Purpose:** Populate GalaxyView with cluster data

**Query Params:**
- `cluster_by` - grouping strategy (default: `jurisdiction`)
  - `jurisdiction` - group by CBU jurisdiction
  - `client` - group by commercial_client_entity_id
  - `product` - group by product_id
  - `risk` - group by risk_context->>'risk_rating'

**Response:** (Matches `UniverseGraph` in taxonomy doc)
```json
{
  "scope": { "type": "universe" },
  "as_of": "2026-01-06",
  "total_cbu_count": 671,
  "cluster_by": "jurisdiction",
  "clusters": [
    {
      "id": "jurisdiction:LU",
      "node_type": "CLUSTER",
      "cluster_type": "JURISDICTION",
      "label": "Luxembourg",
      "short_label": "LU",
      "cbu_count": 177,
      "cbu_ids": ["uuid1", "uuid2", ...],
      "risk_summary": {
        "low": 150,
        "medium": 20,
        "high": 5,
        "unrated": 2
      },
      "suggested_radius": 59.9,
      "suggested_color": "#4CAF50"
    }
  ],
  "cluster_edges": [],
  "stats": {
    "total_clusters": 6,
    "total_cbus": 671,
    "total_entities": 4521,
    "risk_distribution": { "low": 520, "medium": 100, "high": 30, "unrated": 21 }
  }
}
```

**SQL (jurisdiction grouping):**
```sql
SELECT 
    c.jurisdiction as cluster_id,
    c.jurisdiction as cluster_label,
    COUNT(*) as cbu_count,
    ARRAY_AGG(c.cbu_id) as cbu_ids,
    COUNT(*) FILTER (WHERE COALESCE(c.risk_context->>'risk_rating', 'UNRATED') = 'LOW') as low_risk,
    COUNT(*) FILTER (WHERE c.risk_context->>'risk_rating' = 'MEDIUM') as medium_risk,
    COUNT(*) FILTER (WHERE c.risk_context->>'risk_rating' = 'HIGH') as high_risk,
    COUNT(*) FILTER (WHERE COALESCE(c.risk_context->>'risk_rating', 'UNRATED') = 'UNRATED') as unrated
FROM "ob-poc".cbus c
WHERE c.jurisdiction IS NOT NULL
GROUP BY c.jurisdiction
ORDER BY COUNT(*) DESC
```

---

### 2. GET /api/commercial-clients

**Purpose:** List all commercial clients for client picker / book navigation

**Response:**
```json
[
  {
    "entity_id": "allianz-uuid",
    "name": "Allianz SE",
    "lei": "529900K9A2D4NU4G3M46",
    "cbu_count": 47,
    "jurisdictions": ["LU", "IE", "DE"],
    "risk_summary": {
      "low": 40,
      "medium": 5,
      "high": 1,
      "unrated": 1
    }
  }
]
```

**SQL:**
```sql
SELECT 
    e.entity_id,
    COALESCE(lc.registered_name, pp.full_name, 'Unknown') as name,
    lc.lei,
    COUNT(c.cbu_id) as cbu_count,
    ARRAY_AGG(DISTINCT c.jurisdiction) FILTER (WHERE c.jurisdiction IS NOT NULL) as jurisdictions,
    COUNT(*) FILTER (WHERE COALESCE(c.risk_context->>'risk_rating', 'UNRATED') = 'LOW') as low_risk,
    COUNT(*) FILTER (WHERE c.risk_context->>'risk_rating' = 'MEDIUM') as medium_risk,
    COUNT(*) FILTER (WHERE c.risk_context->>'risk_rating' = 'HIGH') as high_risk,
    COUNT(*) FILTER (WHERE COALESCE(c.risk_context->>'risk_rating', 'UNRATED') = 'UNRATED') as unrated
FROM "ob-poc".cbus c
JOIN "ob-poc".entities e ON e.entity_id = c.commercial_client_entity_id
LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.entity_id = e.entity_id
LEFT JOIN "ob-poc".entity_proper_persons pp ON pp.entity_id = e.entity_id
WHERE c.commercial_client_entity_id IS NOT NULL
GROUP BY e.entity_id, lc.registered_name, lc.lei, pp.full_name
ORDER BY COUNT(c.cbu_id) DESC
```

---

### 3. GET /api/commercial-client/:id/book

**Purpose:** Get all CBUs for a commercial client (book view) - maps to `ClusterDetailGraph`

**Response:** (Matches `ClusterDetailGraph` in taxonomy doc)
```json
{
  "cluster": {
    "id": "client:allianz-uuid",
    "node_type": "CLUSTER",
    "cluster_type": "CLIENT",
    "label": "Allianz SE",
    "short_label": "ALZ",
    "cbu_count": 47,
    "cbu_ids": [...],
    "risk_summary": { "low": 40, "medium": 5, "high": 1, "unrated": 1 }
  },
  "cbus": [
    {
      "cbu_id": "fund-a-uuid",
      "name": "Allianz Lux Fund A",
      "jurisdiction": "LU",
      "client_type": "UCITS",
      "risk_rating": "LOW",
      "status": "VALIDATED",
      "entity_count": 23,
      "completion_pct": 0.85,
      "shared_entity_ids": ["manco-uuid", "im-uuid"]
    }
  ],
  "shared_entities": [
    {
      "entity_id": "manco-uuid",
      "name": "Allianz Global Investors GmbH",
      "entity_type": "LIMITED_COMPANY",
      "roles": ["MANAGEMENT_COMPANY"],
      "cbu_count": 35,
      "cbu_ids": [...]
    }
  ],
  "shared_edges": [
    {
      "id": "shared:manco-uuid:fund-a-uuid",
      "shared_entity_id": "manco-uuid",
      "cbu_id": "fund-a-uuid",
      "roles": ["MANAGEMENT_COMPANY"]
    }
  ]
}
```

**SQL (CBUs):**
```sql
SELECT 
    c.cbu_id,
    c.name,
    c.jurisdiction,
    c.client_type,
    c.risk_context->>'risk_rating' as risk_rating,
    c.status
FROM "ob-poc".cbus c
WHERE c.commercial_client_entity_id = $1
ORDER BY c.name
```

**SQL (Shared Entities):**
```sql
WITH client_cbus AS (
    SELECT cbu_id FROM "ob-poc".cbus WHERE commercial_client_entity_id = $1
)
SELECT 
    cer.entity_id,
    COALESCE(lc.registered_name, pp.full_name, 'Unknown') as name,
    ARRAY_AGG(DISTINCT r.name) as roles,
    COUNT(DISTINCT cer.cbu_id) as cbu_count
FROM "ob-poc".cbu_entity_roles cer
JOIN client_cbus cc ON cc.cbu_id = cer.cbu_id
JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
JOIN "ob-poc".roles r ON r.role_id = cer.role_id
LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.entity_id = e.entity_id
LEFT JOIN "ob-poc".entity_proper_persons pp ON pp.entity_id = e.entity_id
GROUP BY cer.entity_id, lc.registered_name, pp.full_name
HAVING COUNT(DISTINCT cer.cbu_id) > 1
ORDER BY COUNT(DISTINCT cer.cbu_id) DESC
```

---

### 4. GET /api/cluster/:cluster_type/:cluster_id/cbus

**Purpose:** Get CBUs for a specific cluster (drill-down from galaxy)

**Examples:**
- `/api/cluster/jurisdiction/LU/cbus` - All Luxembourg CBUs
- `/api/cluster/client/allianz-uuid/cbus` - All Allianz CBUs
- `/api/cluster/risk/HIGH/cbus` - All high-risk CBUs

**Response:**
```json
{
  "cluster_type": "JURISDICTION",
  "cluster_id": "LU",
  "cluster_label": "Luxembourg",
  "cbu_count": 177,
  "cbus": [
    {
      "cbu_id": "uuid",
      "name": "Fund Name",
      "client_name": "Allianz SE",
      "client_type": "UCITS",
      "risk_rating": "LOW"
    }
  ]
}
```

---

## File Structure

```
rust/
├── crates/ob-poc-types/src/
│   └── galaxy.rs                   # NEW - Shared types for server + WASM client
├── src/
│   ├── database/
│   │   ├── mod.rs                  # Add: pub mod universe_repository;
│   │   └── universe_repository.rs  # NEW
│   └── api/
│       ├── mod.rs                  # Add: pub mod universe_routes;
│       └── universe_routes.rs      # NEW
└── main.rs                         # Wire universe router
```

**IMPORTANT:** Response types (`UniverseGraph`, `ClusterNode`, `ClusterDetailGraph`, etc.) should be defined in `ob-poc-types` crate so they're shared between:
- Server (rust/src) - serializes to JSON
- WASM client (ob-poc-ui) - deserializes from JSON

This ensures type alignment and catches breaking changes at compile time.

---

## Implementation: universe_repository.rs

```rust
//! Universe Repository - Queries for galaxy/cluster visualization
//!
//! Provides aggregated views of CBU data grouped by various dimensions
//! (jurisdiction, commercial client, product type, risk rating).

use anyhow::Result;
use sqlx::PgPool;
use uuid::Uuid;

pub struct UniverseRepository {
    pool: PgPool,
}

// ============================================================================
// Row Types
// ============================================================================

#[derive(Debug, sqlx::FromRow)]
pub struct ClusterRow {
    pub cluster_id: String,
    pub cluster_label: String,
    pub cbu_count: i64,
    pub cbu_ids: Vec<Uuid>,
    pub low_risk: i64,
    pub medium_risk: i64,
    pub high_risk: i64,
    pub unrated: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct CommercialClientRow {
    pub entity_id: Uuid,
    pub name: String,
    pub lei: Option<String>,
    pub cbu_count: i64,
    pub jurisdictions: Vec<String>,
    pub low_risk: i64,
    pub medium_risk: i64,
    pub high_risk: i64,
    pub unrated: i64,
}

#[derive(Debug, sqlx::FromRow)]
pub struct CbuSummaryRow {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
    pub risk_rating: Option<String>,
    pub status: Option<String>,
    pub client_name: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct SharedEntityRow {
    pub entity_id: Uuid,
    pub name: String,
    pub roles: Vec<String>,
    pub cbu_count: i64,
}

// ============================================================================
// Implementation
// ============================================================================

impl UniverseRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get CBUs grouped by jurisdiction
    pub async fn clusters_by_jurisdiction(&self) -> Result<Vec<ClusterRow>> {
        let rows = sqlx::query_as::<_, ClusterRow>(r#"
            SELECT 
                c.jurisdiction as cluster_id,
                c.jurisdiction as cluster_label,
                COUNT(*)::bigint as cbu_count,
                ARRAY_AGG(c.cbu_id) as cbu_ids,
                COUNT(*) FILTER (WHERE COALESCE(c.risk_context->>'risk_rating', 'UNRATED') = 'LOW')::bigint as low_risk,
                COUNT(*) FILTER (WHERE c.risk_context->>'risk_rating' = 'MEDIUM')::bigint as medium_risk,
                COUNT(*) FILTER (WHERE c.risk_context->>'risk_rating' = 'HIGH')::bigint as high_risk,
                COUNT(*) FILTER (WHERE COALESCE(c.risk_context->>'risk_rating', 'UNRATED') = 'UNRATED')::bigint as unrated
            FROM "ob-poc".cbus c
            WHERE c.jurisdiction IS NOT NULL
            GROUP BY c.jurisdiction
            ORDER BY COUNT(*) DESC
        "#)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get CBUs grouped by commercial client
    pub async fn clusters_by_client(&self) -> Result<Vec<ClusterRow>> {
        let rows = sqlx::query_as::<_, ClusterRow>(r#"
            SELECT 
                c.commercial_client_entity_id::text as cluster_id,
                COALESCE(lc.registered_name, pp.full_name, 'Unknown') as cluster_label,
                COUNT(*)::bigint as cbu_count,
                ARRAY_AGG(c.cbu_id) as cbu_ids,
                COUNT(*) FILTER (WHERE COALESCE(c.risk_context->>'risk_rating', 'UNRATED') = 'LOW')::bigint as low_risk,
                COUNT(*) FILTER (WHERE c.risk_context->>'risk_rating' = 'MEDIUM')::bigint as medium_risk,
                COUNT(*) FILTER (WHERE c.risk_context->>'risk_rating' = 'HIGH')::bigint as high_risk,
                COUNT(*) FILTER (WHERE COALESCE(c.risk_context->>'risk_rating', 'UNRATED') = 'UNRATED')::bigint as unrated
            FROM "ob-poc".cbus c
            JOIN "ob-poc".entities e ON e.entity_id = c.commercial_client_entity_id
            LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_proper_persons pp ON pp.entity_id = e.entity_id
            WHERE c.commercial_client_entity_id IS NOT NULL
            GROUP BY c.commercial_client_entity_id, lc.registered_name, pp.full_name
            ORDER BY COUNT(*) DESC
        "#)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get CBUs grouped by risk rating
    pub async fn clusters_by_risk(&self) -> Result<Vec<ClusterRow>> {
        let rows = sqlx::query_as::<_, ClusterRow>(r#"
            SELECT 
                COALESCE(c.risk_context->>'risk_rating', 'UNRATED') as cluster_id,
                COALESCE(c.risk_context->>'risk_rating', 'UNRATED') as cluster_label,
                COUNT(*)::bigint as cbu_count,
                ARRAY_AGG(c.cbu_id) as cbu_ids,
                0::bigint as low_risk,
                0::bigint as medium_risk,
                0::bigint as high_risk,
                0::bigint as unrated
            FROM "ob-poc".cbus c
            GROUP BY COALESCE(c.risk_context->>'risk_rating', 'UNRATED')
            ORDER BY 
                CASE COALESCE(c.risk_context->>'risk_rating', 'UNRATED')
                    WHEN 'HIGH' THEN 1
                    WHEN 'MEDIUM' THEN 2
                    WHEN 'LOW' THEN 3
                    ELSE 4
                END
        "#)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// List all commercial clients with CBU counts
    pub async fn list_commercial_clients(&self) -> Result<Vec<CommercialClientRow>> {
        let rows = sqlx::query_as::<_, CommercialClientRow>(r#"
            SELECT 
                e.entity_id,
                COALESCE(lc.registered_name, pp.full_name, 'Unknown') as name,
                lc.lei,
                COUNT(c.cbu_id)::bigint as cbu_count,
                ARRAY_AGG(DISTINCT c.jurisdiction) FILTER (WHERE c.jurisdiction IS NOT NULL) as jurisdictions,
                COUNT(*) FILTER (WHERE COALESCE(c.risk_context->>'risk_rating', 'UNRATED') = 'LOW')::bigint as low_risk,
                COUNT(*) FILTER (WHERE c.risk_context->>'risk_rating' = 'MEDIUM')::bigint as medium_risk,
                COUNT(*) FILTER (WHERE c.risk_context->>'risk_rating' = 'HIGH')::bigint as high_risk,
                COUNT(*) FILTER (WHERE COALESCE(c.risk_context->>'risk_rating', 'UNRATED') = 'UNRATED')::bigint as unrated
            FROM "ob-poc".cbus c
            JOIN "ob-poc".entities e ON e.entity_id = c.commercial_client_entity_id
            LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_proper_persons pp ON pp.entity_id = e.entity_id
            WHERE c.commercial_client_entity_id IS NOT NULL
            GROUP BY e.entity_id, lc.registered_name, lc.lei, pp.full_name
            ORDER BY COUNT(c.cbu_id) DESC
        "#)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get CBUs for a commercial client
    pub async fn cbus_for_client(&self, client_id: Uuid) -> Result<Vec<CbuSummaryRow>> {
        let rows = sqlx::query_as::<_, CbuSummaryRow>(r#"
            SELECT 
                c.cbu_id,
                c.name,
                c.jurisdiction,
                c.client_type,
                c.risk_context->>'risk_rating' as risk_rating,
                c.status,
                NULL::text as client_name
            FROM "ob-poc".cbus c
            WHERE c.commercial_client_entity_id = $1
            ORDER BY c.name
        "#)
        .bind(client_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get CBUs for a jurisdiction
    pub async fn cbus_for_jurisdiction(&self, jurisdiction: &str) -> Result<Vec<CbuSummaryRow>> {
        let rows = sqlx::query_as::<_, CbuSummaryRow>(r#"
            SELECT 
                c.cbu_id,
                c.name,
                c.jurisdiction,
                c.client_type,
                c.risk_context->>'risk_rating' as risk_rating,
                c.status,
                COALESCE(lc.registered_name, pp.full_name) as client_name
            FROM "ob-poc".cbus c
            LEFT JOIN "ob-poc".entities e ON e.entity_id = c.commercial_client_entity_id
            LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_proper_persons pp ON pp.entity_id = e.entity_id
            WHERE c.jurisdiction = $1
            ORDER BY c.name
        "#)
        .bind(jurisdiction)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get shared entities for a commercial client (appear in >1 CBU)
    pub async fn shared_entities_for_client(&self, client_id: Uuid) -> Result<Vec<SharedEntityRow>> {
        let rows = sqlx::query_as::<_, SharedEntityRow>(r#"
            WITH client_cbus AS (
                SELECT cbu_id FROM "ob-poc".cbus WHERE commercial_client_entity_id = $1
            )
            SELECT 
                cer.entity_id,
                COALESCE(lc.registered_name, pp.full_name, 'Unknown') as name,
                ARRAY_AGG(DISTINCT r.name) as roles,
                COUNT(DISTINCT cer.cbu_id)::bigint as cbu_count
            FROM "ob-poc".cbu_entity_roles cer
            JOIN client_cbus cc ON cc.cbu_id = cer.cbu_id
            JOIN "ob-poc".entities e ON e.entity_id = cer.entity_id
            JOIN "ob-poc".roles r ON r.role_id = cer.role_id
            LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.entity_id = e.entity_id
            LEFT JOIN "ob-poc".entity_proper_persons pp ON pp.entity_id = e.entity_id
            GROUP BY cer.entity_id, lc.registered_name, pp.full_name
            HAVING COUNT(DISTINCT cer.cbu_id) > 1
            ORDER BY COUNT(DISTINCT cer.cbu_id) DESC
        "#)
        .bind(client_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Get total CBU count
    pub async fn total_cbu_count(&self) -> Result<i64> {
        let count: (i64,) = sqlx::query_as(r#"SELECT COUNT(*)::bigint FROM "ob-poc".cbus"#)
            .fetch_one(&self.pool)
            .await?;
        Ok(count.0)
    }
}
```

---

## Implementation: universe_routes.rs

```rust
//! Universe API Routes - Galaxy/Cluster visualization endpoints
//!
//! Provides endpoints for:
//! - /api/universe - Get CBUs grouped by cluster dimension
//! - /api/commercial-clients - List commercial clients
//! - /api/commercial-client/:id/book - Get client's CBU book
//! - /api/cluster/:type/:id/cbus - Get CBUs for a cluster

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::database::UniverseRepository;

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct UniverseQuery {
    /// Grouping dimension: jurisdiction (default), client, product, risk
    pub cluster_by: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UniverseResponse {
    pub total_cbu_count: usize,
    pub cluster_by: String,
    pub clusters: Vec<ClusterResponse>,
}

#[derive(Debug, Serialize)]
pub struct ClusterResponse {
    pub id: String,
    pub label: String,
    pub short_label: String,
    pub cbu_count: usize,
    pub cbu_ids: Vec<Uuid>,
    pub cluster_type: String,
    pub risk_summary: RiskSummaryResponse,
}

#[derive(Debug, Serialize)]
pub struct RiskSummaryResponse {
    pub low: usize,
    pub medium: usize,
    pub high: usize,
    pub unrated: usize,
}

#[derive(Debug, Serialize)]
pub struct CommercialClientResponse {
    pub entity_id: Uuid,
    pub name: String,
    pub lei: Option<String>,
    pub cbu_count: usize,
    pub jurisdictions: Vec<String>,
    pub risk_summary: RiskSummaryResponse,
}

#[derive(Debug, Serialize)]
pub struct ClientBookResponse {
    pub client: ClientSummary,
    pub cbu_count: usize,
    pub cbus: Vec<CbuSummaryResponse>,
    pub shared_entities: Vec<SharedEntityResponse>,
}

#[derive(Debug, Serialize)]
pub struct ClientSummary {
    pub entity_id: Uuid,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct CbuSummaryResponse {
    pub cbu_id: Uuid,
    pub name: String,
    pub jurisdiction: Option<String>,
    pub client_type: Option<String>,
    pub risk_rating: Option<String>,
    pub status: Option<String>,
    pub client_name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SharedEntityResponse {
    pub entity_id: Uuid,
    pub name: String,
    pub roles: Vec<String>,
    pub cbu_count: usize,
}

#[derive(Debug, Serialize)]
pub struct ClusterDetailResponse {
    pub cluster_type: String,
    pub cluster_id: String,
    pub cluster_label: String,
    pub cbu_count: usize,
    pub cbus: Vec<CbuSummaryResponse>,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/universe
pub async fn get_universe(
    State(pool): State<PgPool>,
    Query(params): Query<UniverseQuery>,
) -> Result<Json<UniverseResponse>, (StatusCode, String)> {
    let repo = UniverseRepository::new(pool);
    let cluster_by = params.cluster_by.as_deref().unwrap_or("jurisdiction");

    let clusters = match cluster_by {
        "jurisdiction" => repo.clusters_by_jurisdiction().await,
        "client" => repo.clusters_by_client().await,
        "risk" => repo.clusters_by_risk().await,
        _ => repo.clusters_by_jurisdiction().await,
    }
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let total: usize = clusters.iter().map(|c| c.cbu_count as usize).sum();

    let cluster_type = match cluster_by {
        "jurisdiction" => "JURISDICTION",
        "client" => "CLIENT",
        "risk" => "RISK",
        _ => "JURISDICTION",
    };

    let response_clusters: Vec<ClusterResponse> = clusters
        .into_iter()
        .map(|c| {
            let short_label = if c.cluster_label.len() > 4 {
                c.cluster_label.chars().take(2).collect::<String>().to_uppercase()
            } else {
                c.cluster_label.clone()
            };

            ClusterResponse {
                id: c.cluster_id,
                label: c.cluster_label,
                short_label,
                cbu_count: c.cbu_count as usize,
                cbu_ids: c.cbu_ids,
                cluster_type: cluster_type.to_string(),
                risk_summary: RiskSummaryResponse {
                    low: c.low_risk as usize,
                    medium: c.medium_risk as usize,
                    high: c.high_risk as usize,
                    unrated: c.unrated as usize,
                },
            }
        })
        .collect();

    Ok(Json(UniverseResponse {
        total_cbu_count: total,
        cluster_by: cluster_by.to_string(),
        clusters: response_clusters,
    }))
}

/// GET /api/commercial-clients
pub async fn list_commercial_clients(
    State(pool): State<PgPool>,
) -> Result<Json<Vec<CommercialClientResponse>>, (StatusCode, String)> {
    let repo = UniverseRepository::new(pool);

    let clients = repo
        .list_commercial_clients()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let response: Vec<CommercialClientResponse> = clients
        .into_iter()
        .map(|c| CommercialClientResponse {
            entity_id: c.entity_id,
            name: c.name,
            lei: c.lei,
            cbu_count: c.cbu_count as usize,
            jurisdictions: c.jurisdictions,
            risk_summary: RiskSummaryResponse {
                low: c.low_risk as usize,
                medium: c.medium_risk as usize,
                high: c.high_risk as usize,
                unrated: c.unrated as usize,
            },
        })
        .collect();

    Ok(Json(response))
}

/// GET /api/commercial-client/:id/book
pub async fn get_client_book(
    State(pool): State<PgPool>,
    Path(client_id): Path<Uuid>,
) -> Result<Json<ClientBookResponse>, (StatusCode, String)> {
    let repo = UniverseRepository::new(pool);

    // Get client info
    let clients = repo
        .list_commercial_clients()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let client = clients
        .into_iter()
        .find(|c| c.entity_id == client_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Client {} not found", client_id)))?;

    // Get CBUs
    let cbus = repo
        .cbus_for_client(client_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Get shared entities
    let shared = repo
        .shared_entities_for_client(client_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(ClientBookResponse {
        client: ClientSummary {
            entity_id: client.entity_id,
            name: client.name,
        },
        cbu_count: cbus.len(),
        cbus: cbus
            .into_iter()
            .map(|c| CbuSummaryResponse {
                cbu_id: c.cbu_id,
                name: c.name,
                jurisdiction: c.jurisdiction,
                client_type: c.client_type,
                risk_rating: c.risk_rating,
                status: c.status,
                client_name: c.client_name,
            })
            .collect(),
        shared_entities: shared
            .into_iter()
            .map(|s| SharedEntityResponse {
                entity_id: s.entity_id,
                name: s.name,
                roles: s.roles,
                cbu_count: s.cbu_count as usize,
            })
            .collect(),
    }))
}

/// GET /api/cluster/:cluster_type/:cluster_id/cbus
pub async fn get_cluster_cbus(
    State(pool): State<PgPool>,
    Path((cluster_type, cluster_id)): Path<(String, String)>,
) -> Result<Json<ClusterDetailResponse>, (StatusCode, String)> {
    let repo = UniverseRepository::new(pool);

    let (cbus, label) = match cluster_type.as_str() {
        "jurisdiction" => {
            let cbus = repo
                .cbus_for_jurisdiction(&cluster_id)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            (cbus, cluster_id.clone())
        }
        "client" => {
            let client_uuid = Uuid::parse_str(&cluster_id)
                .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid client UUID".to_string()))?;
            let cbus = repo
                .cbus_for_client(client_uuid)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
            // Get client name
            let clients = repo.list_commercial_clients().await.unwrap_or_default();
            let label = clients
                .into_iter()
                .find(|c| c.entity_id == client_uuid)
                .map(|c| c.name)
                .unwrap_or_else(|| cluster_id.clone());
            (cbus, label)
        }
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Unknown cluster type: {}", cluster_type),
            ))
        }
    };

    Ok(Json(ClusterDetailResponse {
        cluster_type: cluster_type.to_uppercase(),
        cluster_id,
        cluster_label: label,
        cbu_count: cbus.len(),
        cbus: cbus
            .into_iter()
            .map(|c| CbuSummaryResponse {
                cbu_id: c.cbu_id,
                name: c.name,
                jurisdiction: c.jurisdiction,
                client_type: c.client_type,
                risk_rating: c.risk_rating,
                status: c.status,
                client_name: c.client_name,
            })
            .collect(),
    }))
}

// ============================================================================
// Router
// ============================================================================

pub fn create_universe_router(pool: PgPool) -> Router {
    Router::new()
        .route("/api/universe", get(get_universe))
        .route("/api/commercial-clients", get(list_commercial_clients))
        .route("/api/commercial-client/:id/book", get(get_client_book))
        .route("/api/cluster/:cluster_type/:cluster_id/cbus", get(get_cluster_cbus))
        .with_state(pool)
}
```

---

## Wiring

### database/mod.rs

Add:
```rust
pub mod universe_repository;
pub use universe_repository::UniverseRepository;
```

### api/mod.rs

Add:
```rust
pub mod universe_routes;
pub use universe_routes::create_universe_router;
```

### main.rs (or router composition)

Add universe router to app:
```rust
let app = Router::new()
    // ... existing routes ...
    .merge(create_universe_router(pool.clone()))
```

---

### 5. GET /api/node/:id/preview (NEW - for inline expansion)

**Purpose:** Lightweight preview data for Esper-style inline expansion without full navigation

**Query Params:**
- `type` - expansion type: `children`, `ownership`, `roles`, `documents`, `appearances`, `history`
- `limit` - max items for children (default: 5)
- `as_of` - temporal filter (default: today)

**Response:**
```json
{
  "node_id": "cluster:LU",
  "node_type": "JURISDICTION_CLUSTER",
  "preview_type": "children",
  "preview_data": {
    "total_count": 177,
    "showing": 5,
    "items": [
      { 
        "id": "cbu:fund-a-uuid", 
        "label": "Allianz Lux Fund A", 
        "node_type": "CBU",
        "risk_rating": "LOW",
        "badges": ["UCITS", "LU"]
      },
      { 
        "id": "cbu:fund-b-uuid", 
        "label": "Allianz Lux Fund B", 
        "node_type": "CBU",
        "risk_rating": "MEDIUM",
        "badges": ["UCITS", "LU"]
      }
    ],
    "has_more": true,
    "expansion_hint": "dive_in_for_full_list"
  }
}
```

**Preview Types:**

| Type | Node Context | Returns |
|------|--------------|---------|
| `children` | Cluster | Top N CBUs sorted by risk/size |
| `children` | CBU | Top N entities by role importance |
| `ownership` | Entity | Upstream chain (max 3 levels) + downstream (max 2) |
| `roles` | Entity | All role assignments with CBU names |
| `documents` | Entity/CBU | Document list with status |
| `appearances` | Entity | CBU list where entity appears |
| `history` | Any | Recent changes (last 5) |

**SQL (children for jurisdiction cluster):**
```sql
SELECT 
    c.cbu_id as id,
    c.name as label,
    'CBU' as node_type,
    COALESCE(c.risk_context->>'risk_rating', 'UNRATED') as risk_rating,
    ARRAY[c.client_type, c.jurisdiction] as badges
FROM "ob-poc".cbus c
WHERE c.jurisdiction = $1
ORDER BY 
    CASE COALESCE(c.risk_context->>'risk_rating', 'UNRATED')
        WHEN 'HIGH' THEN 1
        WHEN 'MEDIUM' THEN 2
        WHEN 'LOW' THEN 3
        ELSE 4
    END,
    c.name
LIMIT $2
```

**SQL (ownership chain for entity):**
```sql
WITH RECURSIVE ownership_chain AS (
    -- Base: the entity itself
    SELECT 
        cer.entity_id,
        cer.target_entity_id as owner_id,
        cer.ownership_percentage,
        1 as depth,
        'upstream' as direction
    FROM "ob-poc".cbu_entity_roles cer
    JOIN "ob-poc".roles r ON r.role_id = cer.role_id
    WHERE cer.entity_id = $1
      AND r.name IN ('SHAREHOLDER', 'BENEFICIAL_OWNER', 'ULTIMATE_BENEFICIAL_OWNER')
      AND cer.target_entity_id IS NOT NULL
    
    UNION ALL
    
    -- Recursive: walk up the chain
    SELECT 
        oc.owner_id as entity_id,
        cer.target_entity_id as owner_id,
        cer.ownership_percentage,
        oc.depth + 1,
        'upstream'
    FROM ownership_chain oc
    JOIN "ob-poc".cbu_entity_roles cer ON cer.entity_id = oc.owner_id
    JOIN "ob-poc".roles r ON r.role_id = cer.role_id
    WHERE r.name IN ('SHAREHOLDER', 'BENEFICIAL_OWNER', 'ULTIMATE_BENEFICIAL_OWNER')
      AND cer.target_entity_id IS NOT NULL
      AND oc.depth < 5  -- Max depth
)
SELECT 
    oc.entity_id,
    oc.owner_id,
    oc.ownership_percentage,
    oc.depth,
    COALESCE(lc.registered_name, pp.full_name, 'Unknown') as owner_name,
    CASE WHEN pp.entity_id IS NOT NULL THEN 'NATURAL_PERSON' ELSE 'LEGAL_ENTITY' END as owner_type
FROM ownership_chain oc
LEFT JOIN "ob-poc".entities e ON e.entity_id = oc.owner_id
LEFT JOIN "ob-poc".entity_limited_companies lc ON lc.entity_id = e.entity_id
LEFT JOIN "ob-poc".entity_proper_persons pp ON pp.entity_id = e.entity_id
ORDER BY oc.depth
```

---

### 6. GET /api/entity/:id/detail (for LOD 3)

**Purpose:** Entity detail view with cross-CBU appearances and ownership chain

**Response:** (Matches `EntityDetailGraph` in taxonomy doc)
```json
{
  "entity": {
    "entity_id": "manco-uuid",
    "entity_type": "LIMITED_COMPANY",
    "name": "Allianz Global Investors GmbH",
    "lei": "529900...",
    "jurisdiction": "DE",
    "incorporation_date": "1998-03-15",
    "status": "ACTIVE",
    "attributes": { ... }
  },
  "cbu_appearances": [
    {
      "cbu_id": "fund-a-uuid",
      "cbu_name": "Allianz Lux Fund A",
      "roles": [
        {
          "role_id": "role-uuid",
          "role_name": "MANAGEMENT_COMPANY",
          "effective_from": "2020-01-01",
          "effective_to": null
        }
      ]
    }
  ],
  "ownership_chain": {
    "upstream": [
      { "entity_id": "parent-uuid", "name": "Allianz SE", "ownership_pct": "100.00", "is_ultimate": true }
    ],
    "downstream": []
  },
  "documents": [
    { "document_id": "doc-uuid", "name": "Certificate of Incorporation", "status": "VERIFIED" }
  ]
}
```

**SQL (CBU appearances):**
```sql
SELECT 
    c.cbu_id,
    c.name as cbu_name,
    json_agg(json_build_object(
        'role_id', r.role_id,
        'role_name', r.name,
        'effective_from', cer.effective_from,
        'effective_to', cer.effective_to
    )) as roles
FROM "ob-poc".cbu_entity_roles cer
JOIN "ob-poc".cbus c ON c.cbu_id = cer.cbu_id
JOIN "ob-poc".roles r ON r.role_id = cer.role_id
WHERE cer.entity_id = $1
GROUP BY c.cbu_id, c.name
ORDER BY c.name
```

---

## Testing

### Manual curl tests:

```bash
# Universe by jurisdiction
curl http://localhost:3000/api/universe

# Universe by client
curl http://localhost:3000/api/universe?cluster_by=client

# Commercial clients list
curl http://localhost:3000/api/commercial-clients

# Client book
curl http://localhost:3000/api/commercial-client/{client-uuid}/book

# Cluster drill-down
curl http://localhost:3000/api/cluster/jurisdiction/LU/cbus
```

### Unit tests (in universe_repository.rs):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Test with actual database (requires test DB setup)
    // Or mock the pool for unit tests
}
```

---

## Checklist

- [ ] Create `ob-poc-types/src/galaxy.rs` with shared response types
- [ ] Create `universe_repository.rs`
- [ ] Create `universe_routes.rs`
- [ ] Add to `database/mod.rs`
- [ ] Add to `api/mod.rs`
- [ ] Wire router in `main.rs`
- [ ] Test `/api/universe` returns clusters (matches `UniverseGraph`)
- [ ] Test `/api/commercial-clients` returns clients
- [ ] Test `/api/commercial-client/:id/book` returns book (matches `ClusterDetailGraph`)
- [ ] Test `/api/cluster/:type/:id/cbus` returns CBUs
- [ ] Test `/api/node/:id/preview` returns inline expansion data
- [ ] Test `/api/entity/:id/detail` returns entity detail (matches `EntityDetailGraph`)
- [ ] Verify response format matches taxonomy doc types exactly
- [ ] Verify ownership chain recursive CTE works for deep hierarchies

---

## Notes

1. **Risk summary per cluster** - Aggregates across all CBUs in cluster
2. **Shared entities** - Only includes entities in >1 CBU for that client
3. **Null handling** - `commercial_client_entity_id` is nullable, excluded from client grouping
4. **Jurisdictions array** - Uses `ARRAY_AGG(DISTINCT ...)` to avoid duplicates
5. **Order** - Clusters ordered by CBU count DESC (biggest first)
