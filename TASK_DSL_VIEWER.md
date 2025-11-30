# TASK: DSL Viewer - Visualize Persisted Agent-Generated DSL

## Goal

Create a server-side DSL display system and minimal UI to visualize:
1. Persisted DSL source code (from agent generation sessions)
2. Compiled execution plans showing dependency ordering
3. Version history for each onboarding workflow

## Context

- Agent generates DSL via prompts → DSL is validated/linted → persisted to DB
- DSL is stored in `dsl_instances` + `dsl_instance_versions` tables
- Key is `business_reference` (e.g., "onboarding-hedge-fund-alpha")
- Existing phase6-web-client is scaffolding for dead service — gut and repurpose

---

## Part 1: Server-Side Implementation

### 1.1 Existing Schema (Reference Only - DO NOT MODIFY)

```sql
-- ob-poc.dsl_instances
CREATE TABLE dsl_instances (
    instance_id UUID PRIMARY KEY,
    domain_name VARCHAR NOT NULL,
    business_reference VARCHAR NOT NULL UNIQUE,
    current_version INT NOT NULL DEFAULT 1,
    status VARCHAR NOT NULL DEFAULT 'ACTIVE',
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
);

-- ob-poc.dsl_instance_versions  
CREATE TABLE dsl_instance_versions (
    version_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    instance_id UUID REFERENCES dsl_instances(instance_id),
    version_number INT NOT NULL,
    dsl_content TEXT NOT NULL,
    operation_type VARCHAR NOT NULL,
    compilation_status VARCHAR NOT NULL DEFAULT 'PENDING',
    ast_json JSONB,
    created_at TIMESTAMPTZ
);
```

### 1.2 New Repository Method

**File:** `rust/src/database/dsl_repository.rs`

Add this struct and method:

```rust
/// Data for DSL visualization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslDisplayData {
    pub instance_id: Uuid,
    pub business_reference: String,
    pub domain_name: String,
    pub version_number: i32,
    pub dsl_content: String,
    pub ast_json: Option<serde_json::Value>,
    pub compilation_status: String,
    pub operation_type: String,
    pub created_at: Option<DateTime<Utc>>,
    /// Computed at display time - list of verb calls in execution order
    pub execution_sequence: Vec<String>,
}

impl DslRepository {
    /// Get DSL for display/visualization
    /// If version is None, returns latest version
    pub async fn get_dsl_for_display(
        &self,
        business_reference: &str,
        version: Option<i32>,
    ) -> Result<Option<DslDisplayData>, sqlx::Error> {
        // Query joins instances + versions
        // If version specified, get that version
        // Otherwise get latest (ORDER BY version_number DESC LIMIT 1)
        // Return None if not found
    }

    /// List all DSL instances for the viewer UI
    pub async fn list_instances_for_display(
        &self,
        limit: Option<i32>,
    ) -> Result<Vec<DslInstanceSummary>, sqlx::Error> {
        // Returns: instance_id, business_reference, domain_name, current_version, updated_at
        // ORDER BY updated_at DESC
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslInstanceSummary {
    pub instance_id: Uuid,
    pub business_reference: String,
    pub domain_name: String,
    pub current_version: i32,
    pub updated_at: Option<DateTime<Utc>>,
}
```

### 1.3 New API Routes

**File:** Create `rust/src/api/dsl_viewer_routes.rs`

```rust
//! DSL Viewer API Routes
//! 
//! Endpoints for visualizing persisted agent-generated DSL
//!
//! Routes:
//! - GET /api/dsl/list              - List all DSL instances
//! - GET /api/dsl/show/:ref         - Get latest DSL for business_reference  
//! - GET /api/dsl/show/:ref/:ver    - Get specific version
//! - GET /api/dsl/history/:ref      - Get all versions for business_reference

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Response types

#[derive(Serialize)]
pub struct DslListResponse {
    pub instances: Vec<DslInstanceSummary>,
    pub total: usize,
}

#[derive(Serialize)]
pub struct DslShowResponse {
    pub business_reference: String,
    pub domain_name: String,
    pub version: i32,
    pub dsl_source: String,
    pub ast_json: Option<serde_json::Value>,
    pub execution_plan: Vec<String>,  // ["cbu.create", "cbu.assign-role", ...]
    pub compilation_status: String,
    pub created_at: Option<String>,
}

#[derive(Serialize)]
pub struct DslHistoryResponse {
    pub business_reference: String,
    pub versions: Vec<DslVersionSummary>,
}

#[derive(Serialize)]
pub struct DslVersionSummary {
    pub version: i32,
    pub operation_type: String,
    pub compilation_status: String,
    pub created_at: Option<String>,
}

// Query params
#[derive(Deserialize)]
pub struct ListQuery {
    pub limit: Option<i32>,
    pub domain: Option<String>,
}

// Route handlers

/// GET /api/dsl/list
async fn list_instances(...) -> Result<Json<DslListResponse>, StatusCode> {
    // Call dsl_repository.list_instances_for_display()
}

/// GET /api/dsl/show/:business_ref
async fn show_dsl(
    Path(business_ref): Path<String>,
) -> Result<Json<DslShowResponse>, StatusCode> {
    // Call dsl_repository.get_dsl_for_display(business_ref, None)
    // If found, also compile the DSL to get execution_plan
    // Use: parse_program() -> compile() -> plan.execution_sequence()
}

/// GET /api/dsl/show/:business_ref/:version
async fn show_dsl_version(
    Path((business_ref, version)): Path<(String, i32)>,
) -> Result<Json<DslShowResponse>, StatusCode> {
    // Same as above but with specific version
}

/// GET /api/dsl/history/:business_ref
async fn dsl_history(
    Path(business_ref): Path<String>,
) -> Result<Json<DslHistoryResponse>, StatusCode> {
    // Call dsl_repository.get_all_versions(business_ref)
    // Map to DslVersionSummary
}

// Router
pub fn dsl_viewer_router() -> Router<AppState> {
    Router::new()
        .route("/list", get(list_instances))
        .route("/show/:business_ref", get(show_dsl))
        .route("/show/:business_ref/:version", get(show_dsl_version))
        .route("/history/:business_ref", get(dsl_history))
}
```

### 1.4 Wire Up Routes

**File:** `rust/src/api/mod.rs`

Add:
```rust
pub mod dsl_viewer_routes;
```

**File:** `rust/src/bin/agentic_server.rs` (or wherever main router is built)

Add under `/api`:
```rust
.nest("/dsl", dsl_viewer_routes::dsl_viewer_router())
```

---

## Part 2: UI Implementation

### 2.1 Approach

**Gut the phase6-web-client** — keep the split-pane layout, replace the guts.

### 2.2 New UI Structure

```
┌─────────────────────────────────────────────────────────────┐
│  DSL Viewer                                    [Refresh]    │
├─────────────────────────────────────────────────────────────┤
│  Onboarding: [dropdown of business_references]              │
│  Version:    [dropdown: v1, v2, v3 (current)]               │
├──────────────────────────┬──────────────────────────────────┤
│                          │                                  │
│   DSL Source             │   Execution Plan                 │
│   ─────────────          │   ──────────────                 │
│                          │                                  │
│   (cbu.create            │   Step 0: cbu.create → @cbu      │
│     :name "Fund"         │   Step 1: cbu.assign-role        │
│     :jurisdiction "US")  │     ← inject cbu-id from $0      │
│                          │   Step 2: cbu.assign-role        │
│   (cbu.assign-role       │     ← inject cbu-id from $0      │
│     :entity-id @john     │                                  │
│     :role "Director")    │                                  │
│                          │                                  │
└──────────────────────────┴──────────────────────────────────┘
│  Status: Loaded v3 | 3 steps | Compiled                     │
└─────────────────────────────────────────────────────────────┘
```

### 2.3 Files to Modify

**`phase6-web-client/src/types.ts`** — Replace types:

```typescript
// Remove old DslDomain, DslVersion types

export interface DslInstance {
  instanceId: string;
  businessReference: string;
  domainName: string;
  currentVersion: number;
  updatedAt: string;
}

export interface DslDisplayData {
  businessReference: string;
  domainName: string;
  version: number;
  dslSource: string;
  astJson: object | null;
  executionPlan: string[];  // ["cbu.create", "cbu.assign-role", ...]
  compilationStatus: string;
  createdAt: string | null;
}

export interface DslVersionInfo {
  version: number;
  operationType: string;
  compilationStatus: string;
  createdAt: string | null;
}

export interface AppState {
  instances: DslInstance[];
  selectedInstance: string | null;  // business_reference
  selectedVersion: number | null;
  displayData: DslDisplayData | null;
  versionHistory: DslVersionInfo[];
  loading: boolean;
  error: string | null;
}
```

**`phase6-web-client/src/api.ts`** — Replace API client:

```typescript
export interface DslViewerApi {
  listInstances(): Promise<DslInstance[]>;
  showDsl(businessRef: string, version?: number): Promise<DslDisplayData>;
  getHistory(businessRef: string): Promise<DslVersionInfo[]>;
}

export class DslViewerApiClient implements DslViewerApi {
  constructor(private baseUrl: string = 'http://localhost:8080/api/dsl') {}

  async listInstances(): Promise<DslInstance[]> {
    const res = await fetch(`${this.baseUrl}/list`);
    const data = await res.json();
    return data.instances;
  }

  async showDsl(businessRef: string, version?: number): Promise<DslDisplayData> {
    const url = version 
      ? `${this.baseUrl}/show/${businessRef}/${version}`
      : `${this.baseUrl}/show/${businessRef}`;
    const res = await fetch(url);
    return res.json();
  }

  async getHistory(businessRef: string): Promise<DslVersionInfo[]> {
    const res = await fetch(`${this.baseUrl}/history/${businessRef}`);
    const data = await res.json();
    return data.versions;
  }
}
```

**`phase6-web-client/src/app.ts`** — Rewrite render logic:

- Left panel: DSL source (syntax highlighted if possible, else textarea)
- Right panel: Execution plan as formatted list showing steps + injections
- Selectors: Instance dropdown, version dropdown
- Status bar: version, step count, compilation status

---

## Part 3: Implementation Order

### Phase A: Server (Do First)
1. [ ] Add `DslDisplayData` and `DslInstanceSummary` structs to `dsl_repository.rs`
2. [ ] Implement `get_dsl_for_display()` method
3. [ ] Implement `list_instances_for_display()` method  
4. [ ] Create `api/dsl_viewer_routes.rs` with all endpoints
5. [ ] Wire up routes in `api/mod.rs` and server binary
6. [ ] Test with curl:
   ```bash
   curl http://localhost:8080/api/dsl/list
   curl http://localhost:8080/api/dsl/show/some-business-ref
   ```

### Phase B: UI (Do Second)
1. [ ] Update `types.ts` with new interfaces
2. [ ] Rewrite `api.ts` with new client
3. [ ] Rewrite `app.ts` render method
4. [ ] Test in browser

---

## Testing

### Create Test Data

Before testing UI, ensure there's DSL in the database. Either:
1. Run an agent session that persists DSL
2. Or insert test data directly:

```sql
INSERT INTO "ob-poc".dsl_instances 
(instance_id, domain_name, business_reference, current_version, status, created_at, updated_at)
VALUES 
(gen_random_uuid(), 'cbu', 'test-hedge-fund-alpha', 1, 'ACTIVE', NOW(), NOW());

-- Then insert a version with actual DSL content
```

### Curl Tests

```bash
# List all instances
curl -s http://localhost:8080/api/dsl/list | jq

# Show specific DSL
curl -s http://localhost:8080/api/dsl/show/test-hedge-fund-alpha | jq

# Show version history
curl -s http://localhost:8080/api/dsl/history/test-hedge-fund-alpha | jq
```

---

## Notes

- The `execution_plan` in the response should be computed by calling `parse_program()` → `compile()` on the stored `dsl_content`, then extracting `plan.execution_sequence()`
- If compilation fails (malformed DSL), return empty execution_plan and set a flag
- Keep the existing `DslRepository` methods — add new ones alongside
- The UI is intentionally minimal — just needs to show DSL and execution order
