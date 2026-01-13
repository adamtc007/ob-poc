# Gap Analysis: CBU Service → Resource Pipeline

> **Created:** 2026-01-13
> **Purpose:** Complete TODO for implementing remaining integration work for the service resource pipeline

---

## Executive Summary

The CBU Service → Resource Discovery → Unified Dictionary → Provisioning pipeline has been **implemented at the core level** but requires integration work to be usable:

| Layer | Status | Gap |
|-------|--------|-----|
| **Database Schema** | ✅ Complete | Migrations 024-027 applied |
| **Rust Types** | ✅ Complete | All domain types in `types.rs` |
| **SRDEF Loader** | ✅ Complete | YAML → registry → DB sync |
| **Discovery Engine** | ✅ Complete | Intent → SRDEF derivation |
| **Rollup Engine** | ✅ Complete | Attr requirement merging |
| **Population Engine** | ✅ Complete | Value sourcing (with stubs) |
| **Provisioning Orchestrator** | ✅ Complete | Topo-sort, request creation |
| **Readiness Engine** | ✅ Complete | Service readiness computation |
| **REST API** | ✅ Complete | Full CRUD + pipeline endpoints |
| **Server Wiring** | ❌ **NOT DONE** | Router not merged into main.rs |
| **MCP Tools** | ❌ **NOT DONE** | No MCP tools for pipeline |
| **DSL Verbs** | ⚠️ Partial | Old verbs exist, new pipeline verbs missing |
| **Taxonomy Viz** | ❌ **NOT DONE** | Product → Service → Resource not visualized |

---

## What Was Built

### 1. Database Migrations (Applied)

| Migration | Purpose |
|-----------|---------|
| `024_service_intents_srdef.sql` | `service_intents`, `srdef_discovery_reasons`, SRDEF columns on `service_resource_types` |
| `025_cbu_unified_attributes.sql` | `cbu_unified_attr_requirements`, `cbu_attr_values`, gap views |
| `026_provisioning_ledger.sql` | `provisioning_requests`, `provisioning_events` (append-only), webhook function |
| `027_service_readiness.sql` | `cbu_service_readiness`, staleness trigger |

### 2. Rust Source Files

#### Core Module: `rust/src/service_resources/`

| File | Lines | Purpose |
|------|-------|---------|
| `mod.rs` | ~100 | Module exports, pipeline documentation |
| `types.rs` | ~600 | All domain types: `ServiceIntent`, `Srdef`, `ProvisioningRequest`, `CbuServiceReadiness`, etc. |
| `service.rs` | ~400 | `ServiceResourcePipelineService` - CRUD operations for all tables |
| `srdef_loader.rs` | ~500 | YAML config parsing, `SrdefRegistry`, DB sync, topo-sort |
| `discovery.rs` | ~500 | `ResourceDiscoveryEngine`, `AttributeRollupEngine`, `PopulationEngine` |
| `provisioning.rs` | ~400 | `ProvisioningOrchestrator`, `ReadinessEngine`, `StubProvisioner` trait |

#### API Routes: `rust/src/api/service_resource_routes.rs`

~500 lines - Full REST API with endpoints:

```
POST /cbu/:cbu_id/service-intents       # Create intent
GET  /cbu/:cbu_id/service-intents       # List intents
GET  /cbu/:cbu_id/service-intents/:id   # Get intent

POST /cbu/:cbu_id/resource-discover     # Run discovery

POST /cbu/:cbu_id/attributes/rollup     # Run rollup
POST /cbu/:cbu_id/attributes/populate   # Run population
GET  /cbu/:cbu_id/attributes/requirements
GET  /cbu/:cbu_id/attributes/values
POST /cbu/:cbu_id/attributes/values     # Set value
GET  /cbu/:cbu_id/attributes/gaps

POST /cbu/:cbu_id/resources/provision   # Run provisioning
GET  /cbu/:cbu_id/provisioning-requests

GET  /cbu/:cbu_id/readiness
POST /cbu/:cbu_id/readiness/recompute

POST /cbu/:cbu_id/pipeline/full         # Run entire pipeline

GET  /srdefs                            # List SRDEFs
GET  /srdefs/:id                        # Get SRDEF detail
```

### 3. SRDEF Configuration: `rust/config/srdefs/`

| File | SRDEFs Defined |
|------|----------------|
| `custody.yaml` | `custody_securities`, `custody_cash`, `settlement_ssi` |
| `connectivity.yaml` | `swift_messaging`, `fix_connectivity`, `api_gateway` |
| `iam.yaml` | `platform_access`, `service_account`, `data_permissions` |

### 4. Stubs / TODOs in Code

In `rust/src/service_resources/discovery.rs`:

```rust
// TODO: Implement document extraction when document store is ready
// TODO: Implement external API lookup when source loaders integrated
// TODO: Implement derivation rules (e.g., entity.jurisdiction → jurisdiction)
// TODO: Implement constraint merging when multiple SRDEFs define same attr
```

These are future features, not blockers.

---

## Gap 1: Server Router Not Wired

**File:** `rust/crates/ob-poc-web/src/main.rs`

**Problem:** The `service_resource_router` exists but is NOT merged into the main Axum app.

**Current state (lines 250-280):**
```rust
let api_router: Router<()> = Router::new()
    .merge(create_agent_router_with_sessions(...))
    .merge(create_attribute_router(...))
    .merge(create_entity_router())
    // ... other routers
    .merge(control_routes(pool.clone()));
    // ❌ MISSING: service_resource_router
```

**Fix required:**

1. Add import in `rust/src/api/mod.rs`:
```rust
mod service_resource_routes;
pub use service_resource_routes::service_resource_router;
```

2. Merge into main.rs:
```rust
use ob_poc::api::service_resource_router;
// ...
.merge(service_resource_router(pool.clone()))
```

3. Add to startup log:
```rust
tracing::info!("  /api/cbu/:id/service-intents - Service intents");
tracing::info!("  /api/cbu/:id/readiness       - Service readiness");
tracing::info!("  /api/srdefs                  - SRDEF registry");
```

---

## Gap 2: No MCP Tools

**File:** `rust/src/mcp/tools.rs`

**Problem:** The MCP server has no tools for the service resource pipeline. Agents cannot:
- Create/list service intents
- Run discovery/rollup/population
- Check readiness
- Trigger provisioning

**Required MCP tools:**

| Tool | Description |
|------|-------------|
| `service_intent_create` | Create service intent for CBU |
| `service_intent_list` | List service intents for CBU |
| `service_discovery_run` | Run resource discovery for CBU |
| `service_attributes_gaps` | Get attribute gaps for CBU |
| `service_attributes_set` | Set attribute value |
| `service_readiness_get` | Get readiness status for CBU |
| `service_readiness_recompute` | Force readiness recomputation |
| `service_pipeline_run` | Run full pipeline (discovery → provision → readiness) |
| `srdef_list` | List available SRDEFs |
| `srdef_get` | Get SRDEF details |

**Implementation location:**

1. Add tool definitions to `rust/src/mcp/tools.rs` in `get_tools()` function
2. Add handlers to `rust/src/mcp/handlers/core.rs`

---

## Gap 3: DSL Verbs Incomplete

**Existing file:** `rust/config/verbs/service-resource.yaml`

**Current state:** Old verbs for direct resource provisioning:
- `service-resource.provision`
- `service-resource.set-attr`
- `service-resource.activate`
- `service-resource.suspend`
- `service-resource.decommission`

**Missing verbs for new pipeline:**

| Domain | Verb | Purpose |
|--------|------|---------|
| `service-intent` | `create` | Create service intent |
| `service-intent` | `list` | List intents for CBU |
| `service-intent` | `supersede` | Supersede an intent |
| `discovery` | `run` | Run resource discovery |
| `discovery` | `explain` | Show why SRDEFs were discovered |
| `attributes` | `rollup` | Run attribute rollup |
| `attributes` | `populate` | Run attribute population |
| `attributes` | `gaps` | Show attribute gaps |
| `attributes` | `set` | Set attribute value |
| `readiness` | `compute` | Compute readiness |
| `readiness` | `explain` | Show blocking reasons |
| `provisioning` | `run` | Run provisioning orchestrator |
| `pipeline` | `full` | Run entire pipeline |

**Implementation:**

Create new YAML file `rust/config/verbs/service-pipeline.yaml`:

```yaml
domains:
  service-intent:
    description: "Service intent management"
    verbs:
      create:
        description: "Declare CBU wants a product+service"
        metadata:
          tier: intent
          source_of_truth: operational
          scope: cbu
          noun: service_intent
        behavior: plugin
        handler: ServiceIntentCreateOp
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: product-id
            type: uuid
            required: true
            lookup:
              table: products
              schema: ob-poc
              search_key: name
              primary_key: product_id
          - name: service-id
            type: uuid
            required: true
            lookup:
              table: services
              schema: ob-poc
              search_key: name
              primary_key: service_id
          - name: options
            type: json
            required: false
        returns:
          type: uuid
          name: intent_id
          capture: true

  discovery:
    description: "Resource discovery operations"
    verbs:
      run:
        description: "Run resource discovery for CBU"
        metadata:
          tier: composite
          source_of_truth: operational
          scope: cbu
          noun: discovery
        behavior: plugin
        handler: DiscoveryRunOp
        args:
          - name: cbu-id
            type: uuid
            required: true
        returns:
          type: record
          name: discovery_result

  readiness:
    description: "Service readiness operations"
    verbs:
      compute:
        description: "Compute service readiness for CBU"
        metadata:
          tier: composite
          source_of_truth: operational
          scope: cbu
          noun: readiness
        behavior: plugin
        handler: ReadinessComputeOp
        args:
          - name: cbu-id
            type: uuid
            required: true
        returns:
          type: record
          name: readiness_result

      explain:
        description: "Explain blocking reasons for service"
        metadata:
          tier: diagnostics
          source_of_truth: operational
          scope: cbu
          noun: readiness
        behavior: plugin
        handler: ReadinessExplainOp
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: service-id
            type: uuid
            required: true
        returns:
          type: record
          name: blocking_reasons
```

**Plugin handlers needed:**
- `ServiceIntentCreateOp` → `rust/src/dsl_v2/custom_ops/service_intent.rs`
- `DiscoveryRunOp` → `rust/src/dsl_v2/custom_ops/discovery.rs`
- `ReadinessComputeOp` → `rust/src/dsl_v2/custom_ops/readiness.rs`
- `ReadinessExplainOp` → same file

---

## Gap 4: No Taxonomy Visualization

**Problem:** The UI has no way to visualize the Product → Service → Resource taxonomy as a navigable hierarchy.

**Current UI taxonomy:** Uses `TaxonomyStack` for entity drilling (fund → subfund → entity graph).

**Required:** Similar navigation for:
```
Products
  └── Global Custody
       └── Securities Services
            └── custody_securities (SRDEF)
                 ├── settlement_ssi
                 └── swift_messaging
       └── Cash Management
            └── custody_cash (SRDEF)
  └── Trading
       └── Execution
            └── fix_connectivity (SRDEF)
       └── Clearing
            └── ...
```

**Implementation approach:**

1. **Data model:** Already exists - `products` → `product_services` → `services` → `service_resource_capabilities` → `service_resource_types`

2. **API endpoint needed:**
   ```
   GET /api/taxonomy/products          # List products
   GET /api/taxonomy/products/:id/services  # Services for product
   GET /api/taxonomy/services/:id/resources # Resources for service
   GET /api/taxonomy/srdefs/:id/dependencies # SRDEF dependency tree
   ```

3. **UI component:** New panel or mode in existing taxonomy navigator

4. **Graph integration:** Show resource dependency graph (from SRDEF `depends_on`)

**Files to modify:**
- `rust/src/api/` - Add taxonomy_product_routes.rs
- `rust/crates/ob-poc-ui/src/panels/` - Add product_taxonomy.rs or extend taxonomy.rs
- `rust/crates/ob-poc-graph/src/` - Add product/service graph layout

---

## Implementation Order (Recommended)

### Phase 1: Server Wiring (30 min)
1. Export router from `rust/src/api/mod.rs`
2. Merge into `rust/crates/ob-poc-web/src/main.rs`
3. Test endpoints work

### Phase 2: MCP Tools (2-3 hours)
1. Add 10 tool definitions to `tools.rs`
2. Add handlers to `handlers/core.rs`
3. Test via MCP protocol

### Phase 3: DSL Verbs (3-4 hours)
1. Create `rust/config/verbs/service-pipeline.yaml`
2. Create plugin handlers in `rust/src/dsl_v2/custom_ops/`
3. Register handlers in `verb_registry.rs`
4. Test via CLI: `cargo run -p ob-poc-cli -- execute "discovery.run cbu-id=..."`

### Phase 4: Taxonomy Visualization (4-6 hours)
1. Add product taxonomy API routes
2. Add UI panel or extend existing
3. Wire up navigation
4. Add resource dependency graph

---

## Key Source Files Reference

### Implemented (Read for Context)

| File | Purpose |
|------|---------|
| `rust/src/service_resources/mod.rs` | Pipeline overview, module exports |
| `rust/src/service_resources/types.rs` | All domain types |
| `rust/src/service_resources/service.rs` | Database operations |
| `rust/src/service_resources/srdef_loader.rs` | YAML → registry loader |
| `rust/src/service_resources/discovery.rs` | Discovery + rollup + population engines |
| `rust/src/service_resources/provisioning.rs` | Provisioning orchestrator + readiness |
| `rust/src/api/service_resource_routes.rs` | REST API handlers |
| `rust/config/srdefs/*.yaml` | SRDEF definitions |

### To Modify

| File | Change |
|------|--------|
| `rust/src/api/mod.rs` | Add `pub use service_resource_routes::service_resource_router` |
| `rust/crates/ob-poc-web/src/main.rs` | Merge `service_resource_router` |
| `rust/src/mcp/tools.rs` | Add 10 tool definitions |
| `rust/src/mcp/handlers/core.rs` | Add tool handlers |
| `rust/config/verbs/service-pipeline.yaml` | **NEW FILE** - DSL verbs |
| `rust/src/dsl_v2/custom_ops/` | **NEW FILES** - Plugin handlers |

### Database Tables (Already Created)

| Table | Schema | Purpose |
|-------|--------|---------|
| `service_intents` | ob-poc | What CBU wants |
| `srdef_discovery_reasons` | ob-poc | Why SRDEFs derived |
| `cbu_unified_attr_requirements` | ob-poc | Rolled-up attrs |
| `cbu_attr_values` | ob-poc | Attr values |
| `provisioning_requests` | ob-poc | Request log |
| `provisioning_events` | ob-poc | Event log (append-only) |
| `cbu_service_readiness` | ob-poc | Readiness status |

---

## Testing Checklist

After implementation, verify:

- [ ] `GET /api/srdefs` returns loaded SRDEFs
- [ ] `POST /api/cbu/:id/service-intents` creates intent
- [ ] `POST /api/cbu/:id/resource-discover` discovers SRDEFs
- [ ] `GET /api/cbu/:id/attributes/gaps` shows missing attrs
- [ ] `POST /api/cbu/:id/pipeline/full` runs entire pipeline
- [ ] `GET /api/cbu/:id/readiness` shows service status
- [ ] MCP tool `service_readiness_get` works via Claude
- [ ] DSL verb `discovery.run cbu-id=...` executes
- [ ] Taxonomy viz shows Product → Service → Resource hierarchy

---

## Notes

1. **Stubs in discovery.rs** are for future features (document extraction, external API, derivation rules). Not blockers.

2. **Existing `service-resource.yaml` verbs** are for direct resource management. New pipeline verbs complement, don't replace.

3. **SrdefRegistry** loads on server startup. Changes to YAML require restart.

4. **Append-only ledger** - `provisioning_events` has trigger preventing UPDATE/DELETE. Use `process_provisioning_result()` function for webhook handling.
