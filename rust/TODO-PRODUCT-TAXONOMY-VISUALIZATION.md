# TODO: Product Taxonomy Visualization - CBU Container Navigation

## Overview

CBU container should offer **three navigation paths** from the CBU node:
1. **Entity Graph** (current default) - entities inside the CBU
2. **Instrument Matrix** - hierarchical custody configuration (exists, needs wiring)
3. **Product Set** - Product → Service → Resource taxonomy (needs implementation)

```
CBU Container
    ├── [👥] Entity Graph (current - entities, relationships)
    ├── [📊] Instrument Matrix (exists - TradingMatrix)
    └── [📦] Product Set (missing - ServiceTaxonomy)
```

---

## Gap Analysis

| Layer | Status | Gap |
|-------|--------|-----|
| **ServiceTaxonomy struct** | ✅ Complete | `ob-poc-graph/src/graph/service_taxonomy.rs` |
| **ServiceTaxonomyState** | ✅ Complete | UI expand/collapse, selection state |
| **service_taxonomy_panel()** | ✅ Complete | `ob-poc-ui/src/panels/service_taxonomy.rs` |
| **render_service_taxonomy()** | ✅ Complete | Renderer in `ob-poc-graph` |
| **state.service_taxonomy** | ✅ Declared | Field exists in `AppState`, never populated |
| **API endpoint** | ❌ **MISSING** | Need `/api/cbu/:id/service-taxonomy` |
| **API client call** | ❌ **MISSING** | Need `get_service_taxonomy()` in `api.rs` |
| **Fetch trigger** | ❌ **MISSING** | Nothing calls the API |
| **CBU nav icons** | ❌ **MISSING** | No Entity/Matrix/Product switch UI |

---

## Phase 1: Server Endpoint (2h)

### 1.1 Create Service Taxonomy API Response Type

**File:** `rust/src/api/service_resource_routes.rs`

Add response types that match what `ServiceTaxonomy::from_data()` expects:

```rust
/// Response for GET /cbu/:id/service-taxonomy
#[derive(Debug, Serialize)]
pub struct ServiceTaxonomyResponse {
    pub cbu_id: Uuid,
    pub cbu_name: String,
    pub products: Vec<ProductTaxonomyNode>,
    pub stats: ServiceTaxonomyStats,
}

#[derive(Debug, Serialize)]
pub struct ProductTaxonomyNode {
    pub product_id: Uuid,
    pub name: String,
    pub services: Vec<ServiceTaxonomyNode>,
}

#[derive(Debug, Serialize)]
pub struct ServiceTaxonomyNode {
    pub service_id: Uuid,
    pub name: String,
    pub status: String, // "ready", "partial", "blocked", "pending"
    pub blocking_reasons: Vec<String>,
    pub attr_progress: Option<(usize, usize)>, // (satisfied, total)
    pub intents: Vec<IntentTaxonomyNode>,
}

#[derive(Debug, Serialize)]
pub struct IntentTaxonomyNode {
    pub intent_id: Uuid,
    pub options_summary: Option<String>,
    pub resources: Vec<ResourceTaxonomyNode>,
}

#[derive(Debug, Serialize)]
pub struct ResourceTaxonomyNode {
    pub srdef_id: String,
    pub name: String,
    pub resource_type: String,
    pub status: String,
    pub blocking_reasons: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ServiceTaxonomyStats {
    pub total_products: usize,
    pub total_services: usize,
    pub total_resources: usize,
    pub ready_count: usize,
    pub blocked_count: usize,
}
```

### 1.2 Add Endpoint Handler

**File:** `rust/src/api/service_resource_routes.rs`

```rust
/// GET /cbu/:cbu_id/service-taxonomy
async fn get_service_taxonomy(
    Path(cbu_id): Path<Uuid>,
    State(state): State<Arc<ServiceResourceState>>,
) -> Result<Json<ServiceTaxonomyResponse>, (StatusCode, Json<serde_json::Value>)> {
    // 1. Get CBU name
    let cbu = sqlx::query!(
        "SELECT name FROM cbus WHERE cbu_id = $1",
        cbu_id
    )
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| db_error(e))?
    .ok_or_else(|| not_found("CBU"))?;

    // 2. Get products for this CBU (from cbu_products or product_services)
    let products = sqlx::query!(
        r#"
        SELECT DISTINCT p.product_id, p.name
        FROM products p
        JOIN service_intents si ON si.product_id = p.product_id
        WHERE si.cbu_id = $1 AND si.superseded_at IS NULL
        ORDER BY p.name
        "#,
        cbu_id
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| db_error(e))?;

    let mut product_nodes = Vec::new();

    for product in products {
        // 3. Get services for this product
        let services = sqlx::query!(
            r#"
            SELECT DISTINCT s.service_id, s.name
            FROM services s
            JOIN service_intents si ON si.service_id = s.service_id
            WHERE si.cbu_id = $1 AND si.product_id = $2 AND si.superseded_at IS NULL
            ORDER BY s.name
            "#,
            cbu_id, product.product_id
        )
        .fetch_all(&state.pool)
        .await
        .map_err(|e| db_error(e))?;

        let mut service_nodes = Vec::new();

        for service in services {
            // 4. Get readiness for this service
            let readiness = sqlx::query!(
                r#"
                SELECT status, blocking_reasons
                FROM cbu_service_readiness
                WHERE cbu_id = $1 AND product_id = $2 AND service_id = $3
                "#,
                cbu_id, product.product_id, service.service_id
            )
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| db_error(e))?;

            let (status, blocking_reasons) = readiness
                .map(|r| (r.status, r.blocking_reasons.unwrap_or_default()))
                .unwrap_or(("pending".to_string(), vec![]));

            // 5. Get intents for this service
            let intents = sqlx::query!(
                r#"
                SELECT intent_id, options
                FROM service_intents
                WHERE cbu_id = $1 AND product_id = $2 AND service_id = $3
                  AND superseded_at IS NULL
                "#,
                cbu_id, product.product_id, service.service_id
            )
            .fetch_all(&state.pool)
            .await
            .map_err(|e| db_error(e))?;

            let mut intent_nodes = Vec::new();

            for intent in intents {
                // 6. Get discovered SRDEFs for this intent
                let resources = sqlx::query!(
                    r#"
                    SELECT sdr.srdef_id, srt.name, srt.resource_kind,
                           sri.state as instance_state
                    FROM srdef_discovery_reasons sdr
                    JOIN service_resource_types srt ON sdr.srdef_id = srt.srdef_id
                    LEFT JOIN service_resource_instances sri 
                        ON sri.cbu_id = sdr.cbu_id AND sri.srdef_id = sdr.srdef_id
                    WHERE sdr.cbu_id = $1 AND sdr.intent_id = $2
                    "#,
                    cbu_id, intent.intent_id
                )
                .fetch_all(&state.pool)
                .await
                .map_err(|e| db_error(e))?;

                let resource_nodes: Vec<ResourceTaxonomyNode> = resources
                    .into_iter()
                    .map(|r| ResourceTaxonomyNode {
                        srdef_id: r.srdef_id,
                        name: r.name,
                        resource_type: r.resource_kind,
                        status: r.instance_state.unwrap_or("pending".to_string()),
                        blocking_reasons: vec![], // TODO: fetch from instance
                    })
                    .collect();

                intent_nodes.push(IntentTaxonomyNode {
                    intent_id: intent.intent_id,
                    options_summary: intent.options.map(|o| format!("{}", o)),
                    resources: resource_nodes,
                });
            }

            // 7. Calculate attr progress
            let attr_stats = sqlx::query!(
                r#"
                SELECT 
                    COUNT(*) as total,
                    COUNT(cav.attr_id) as satisfied
                FROM cbu_unified_attr_requirements cuar
                LEFT JOIN cbu_attr_values cav 
                    ON cuar.cbu_id = cav.cbu_id AND cuar.attr_id = cav.attr_id
                WHERE cuar.cbu_id = $1
                "#,
                cbu_id
            )
            .fetch_one(&state.pool)
            .await
            .map_err(|e| db_error(e))?;

            service_nodes.push(ServiceTaxonomyNode {
                service_id: service.service_id,
                name: service.name,
                status,
                blocking_reasons,
                attr_progress: Some((
                    attr_stats.satisfied.unwrap_or(0) as usize,
                    attr_stats.total.unwrap_or(0) as usize,
                )),
                intents: intent_nodes,
            });
        }

        product_nodes.push(ProductTaxonomyNode {
            product_id: product.product_id,
            name: product.name,
            services: service_nodes,
        });
    }

    // 8. Calculate stats
    let total_services = product_nodes.iter().map(|p| p.services.len()).sum();
    let total_resources = product_nodes
        .iter()
        .flat_map(|p| p.services.iter())
        .flat_map(|s| s.intents.iter())
        .map(|i| i.resources.len())
        .sum();
    let ready_count = product_nodes
        .iter()
        .flat_map(|p| p.services.iter())
        .filter(|s| s.status == "ready")
        .count();
    let blocked_count = product_nodes
        .iter()
        .flat_map(|p| p.services.iter())
        .filter(|s| s.status == "blocked")
        .count();

    Ok(Json(ServiceTaxonomyResponse {
        cbu_id,
        cbu_name: cbu.name,
        products: product_nodes,
        stats: ServiceTaxonomyStats {
            total_products: product_nodes.len(),
            total_services,
            total_resources,
            ready_count,
            blocked_count,
        },
    }))
}
```

### 1.3 Wire Route

```rust
// In service_resource_router():
.route("/cbu/:cbu_id/service-taxonomy", get(get_service_taxonomy))
```

---

## Phase 2: API Client (30min)

### 2.1 Add API Call

**File:** `rust/crates/ob-poc-ui/src/api.rs`

```rust
use ob_poc_graph::ServiceTaxonomy;

/// Response from service taxonomy endpoint
#[derive(Clone, Debug, serde::Deserialize)]
pub struct ServiceTaxonomyResponse {
    pub cbu_id: Uuid,
    pub cbu_name: String,
    pub products: Vec<ProductTaxonomyNode>,
    pub stats: ServiceTaxonomyStats,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct ProductTaxonomyNode {
    pub product_id: Uuid,
    pub name: String,
    pub services: Vec<ServiceTaxonomyNodeResponse>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct ServiceTaxonomyNodeResponse {
    pub service_id: Uuid,
    pub name: String,
    pub status: String,
    pub blocking_reasons: Vec<String>,
    pub attr_progress: Option<(usize, usize)>,
    pub intents: Vec<IntentTaxonomyNode>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct IntentTaxonomyNode {
    pub intent_id: Uuid,
    pub options_summary: Option<String>,
    pub resources: Vec<ResourceTaxonomyNode>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct ResourceTaxonomyNode {
    pub srdef_id: String,
    pub name: String,
    pub resource_type: String,
    pub status: String,
    pub blocking_reasons: Vec<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct ServiceTaxonomyStats {
    pub total_products: usize,
    pub total_services: usize,
    pub total_resources: usize,
    pub ready_count: usize,
    pub blocked_count: usize,
}

/// Get service taxonomy (Product → Service → Resource hierarchy) for a CBU
pub async fn get_service_taxonomy(cbu_id: Uuid) -> Result<ServiceTaxonomy, String> {
    let response: ServiceTaxonomyResponse = 
        get(&format!("/api/cbu/{}/service-taxonomy", cbu_id)).await?;
    
    // Convert API response to ServiceTaxonomy using the from_data() builder
    use ob_poc_graph::{ProductData, ServiceData, IntentData, ResourceData, ServiceStatus};
    
    let products: Vec<ProductData> = response.products.into_iter().map(|p| {
        ProductData {
            id: p.product_id,
            name: p.name,
            services: p.services.into_iter().map(|s| {
                ServiceData {
                    id: s.service_id,
                    name: s.name,
                    status: match s.status.as_str() {
                        "ready" => ServiceStatus::Ready,
                        "partial" => ServiceStatus::Partial,
                        "blocked" => ServiceStatus::Blocked,
                        _ => ServiceStatus::Pending,
                    },
                    blocking_reasons: s.blocking_reasons,
                    attr_progress: s.attr_progress,
                    intents: s.intents.into_iter().map(|i| {
                        IntentData {
                            id: i.intent_id,
                            options_summary: i.options_summary,
                            resources: i.resources.into_iter().map(|r| {
                                ResourceData {
                                    srdef_id: r.srdef_id,
                                    name: r.name,
                                    resource_type: r.resource_type,
                                    status: match r.status.as_str() {
                                        "active" => ServiceStatus::Ready,
                                        "provisioning" => ServiceStatus::Partial,
                                        "failed" => ServiceStatus::Blocked,
                                        _ => ServiceStatus::Pending,
                                    },
                                    blocking_reasons: r.blocking_reasons,
                                }
                            }).collect(),
                        }
                    }).collect(),
                }
            }).collect(),
        }
    }).collect();
    
    Ok(ServiceTaxonomy::from_data(
        response.cbu_id,
        &response.cbu_name,
        products,
    ))
}
```

---

## Phase 3: Fetch Trigger & State Update (1h)

### 3.1 Add to AsyncState

**File:** `rust/crates/ob-poc-ui/src/state.rs`

In `AsyncState`:
```rust
/// Service taxonomy fetch result
pub service_taxonomy_result: Option<Result<ServiceTaxonomy, String>>,
/// Loading flag
pub loading_service_taxonomy: bool,
```

### 3.2 Add Fetch Trigger

When CBU is selected AND user navigates to "Product Set" view, trigger fetch:

**File:** `rust/crates/ob-poc-ui/src/app.rs` (or wherever CBU selection is handled)

```rust
// When user selects Product Set view mode for current CBU:
if let Some(cbu_id) = state.session.as_ref().and_then(|s| s.active_cbu_id) {
    if let Ok(mut async_state) = state.async_state.lock() {
        if !async_state.loading_service_taxonomy {
            async_state.loading_service_taxonomy = true;
            
            wasm_bindgen_futures::spawn_local(async move {
                let result = api::get_service_taxonomy(cbu_id).await;
                // Store result in async_state...
            });
        }
    }
}
```

### 3.3 Update State from Async Result

In the main update loop:
```rust
// Check for service taxonomy result
if let Some(result) = async_state.service_taxonomy_result.take() {
    async_state.loading_service_taxonomy = false;
    match result {
        Ok(taxonomy) => {
            state.service_taxonomy = Some(taxonomy);
        }
        Err(e) => {
            web_sys::console::error_1(&format!("Service taxonomy fetch failed: {}", e).into());
        }
    }
}
```

---

## Phase 4: CBU Container Navigation Icons (2h)

### 4.1 Add Navigation Mode Enum

**File:** `rust/crates/ob-poc-ui/src/state.rs` (or new `navigation.rs`)

```rust
/// Navigation mode when viewing a CBU
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CbuViewMode {
    /// Entity relationship graph (default)
    #[default]
    EntityGraph,
    /// Instrument matrix (trading profile hierarchy)
    InstrumentMatrix,
    /// Product set (service taxonomy)
    ProductSet,
}
```

Add to `AppState`:
```rust
/// Current CBU view mode
pub cbu_view_mode: CbuViewMode,
```

### 4.2 Render Navigation Icons

**File:** `rust/crates/ob-poc-ui/src/panels/toolbar.rs` (or new widget)

```rust
/// Render CBU view mode selector
/// Returns Some(mode) if user clicked a different mode
pub fn cbu_view_selector(ui: &mut egui::Ui, current: CbuViewMode) -> Option<CbuViewMode> {
    let mut action = None;
    
    ui.horizontal(|ui| {
        // Entity Graph button
        let entity_btn = ui.selectable_label(
            current == CbuViewMode::EntityGraph,
            "👥 Entities"
        );
        if entity_btn.clicked() && current != CbuViewMode::EntityGraph {
            action = Some(CbuViewMode::EntityGraph);
        }
        entity_btn.on_hover_text("Entity relationship graph");
        
        ui.separator();
        
        // Instrument Matrix button  
        let matrix_btn = ui.selectable_label(
            current == CbuViewMode::InstrumentMatrix,
            "📊 Matrix"
        );
        if matrix_btn.clicked() && current != CbuViewMode::InstrumentMatrix {
            action = Some(CbuViewMode::InstrumentMatrix);
        }
        matrix_btn.on_hover_text("Instrument/custody matrix");
        
        ui.separator();
        
        // Product Set button
        let product_btn = ui.selectable_label(
            current == CbuViewMode::ProductSet,
            "📦 Products"
        );
        if product_btn.clicked() && current != CbuViewMode::ProductSet {
            action = Some(CbuViewMode::ProductSet);
        }
        product_btn.on_hover_text("Product → Service → Resource taxonomy");
    });
    
    action
}
```

### 4.3 Wire View Mode Switching

In main render loop, when CBU is active:

```rust
// Render view mode selector
if state.session.as_ref().map(|s| s.active_cbu_id.is_some()).unwrap_or(false) {
    if let Some(new_mode) = cbu_view_selector(ui, state.cbu_view_mode) {
        state.cbu_view_mode = new_mode;
        
        // Trigger data fetch for new mode
        match new_mode {
            CbuViewMode::EntityGraph => {
                // Already loaded with graph_data
            }
            CbuViewMode::InstrumentMatrix => {
                // Trigger trading matrix fetch if not loaded
                trigger_trading_matrix_fetch(state);
            }
            CbuViewMode::ProductSet => {
                // Trigger service taxonomy fetch if not loaded
                trigger_service_taxonomy_fetch(state);
            }
        }
    }
}

// Render appropriate panel based on mode
match state.cbu_view_mode {
    CbuViewMode::EntityGraph => {
        // Render entity graph (existing code)
        render_cbu_graph(ui, state);
    }
    CbuViewMode::InstrumentMatrix => {
        // Render trading matrix panel
        trading_matrix_panel(ui, state, max_height);
    }
    CbuViewMode::ProductSet => {
        // Render service taxonomy panel
        service_taxonomy_panel(ui, state, max_height);
    }
}
```

---

## Phase 5: Zoom Navigation for Product Taxonomy (1h)

### 5.1 Product Taxonomy Zoom Actions

The `ServiceTaxonomyAction` enum already supports drill-down:

```rust
pub enum ServiceTaxonomyAction {
    ToggleExpand { node_id: ServiceTaxonomyNodeId },
    SelectNode { node_id: ServiceTaxonomyNodeId },
    DrillIntoResource { srdef_id: String },
    // ... etc
}
```

### 5.2 Handle Drill-Down in Main Loop

```rust
// Handle service taxonomy actions
if let ServiceTaxonomyPanelAction::DrillIntoResource { srdef_id } = taxonomy_action {
    // Navigate to resource detail view
    // This could open a detail panel or navigate to the resource in another context
    state.selected_srdef = Some(srdef_id);
}
```

---

## Testing Checklist

### Phase 1: Server
- [ ] `curl http://localhost:3000/api/cbu/{cbu_id}/service-taxonomy | jq` returns hierarchy
- [ ] Products contain services
- [ ] Services contain intents + readiness status
- [ ] Intents contain discovered SRDEFs

### Phase 2-3: API & State
- [ ] `get_service_taxonomy()` returns `ServiceTaxonomy`
- [ ] `state.service_taxonomy` populates when CBU selected + Product Set mode
- [ ] Loading spinner shows while fetching

### Phase 4: Navigation
- [ ] CBU view shows Entity / Matrix / Products selector
- [ ] Clicking Products switches to service_taxonomy_panel
- [ ] Clicking Matrix switches to trading_matrix_panel
- [ ] Clicking Entities returns to graph view

### Phase 5: Drill-Down
- [ ] Expand/collapse nodes works
- [ ] Drill into resource shows detail
- [ ] Status colors show correctly (ready/partial/blocked)

---

## Files Summary

### New/Modified Server Files
```
rust/src/api/service_resource_routes.rs  - Add endpoint + response types
```

### New/Modified UI Files
```
rust/crates/ob-poc-ui/src/api.rs         - Add get_service_taxonomy()
rust/crates/ob-poc-ui/src/state.rs       - Add CbuViewMode, async state fields
rust/crates/ob-poc-ui/src/app.rs         - Wire view mode switching
rust/crates/ob-poc-ui/src/panels/toolbar.rs  - Add cbu_view_selector widget
```

---

## Total Effort

| Phase | Time | Priority |
|-------|------|----------|
| Phase 1: Server Endpoint | 2h | P0 |
| Phase 2: API Client | 30min | P0 |
| Phase 3: Fetch Trigger | 1h | P0 |
| Phase 4: Navigation Icons | 2h | P1 |
| Phase 5: Zoom Navigation | 1h | P2 |
| **Total** | **~6.5h** | |
