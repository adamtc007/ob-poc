# TODO: Service Resource Pipeline - Wiring & Integration

## Overview

The core pipeline (types, engines, API routes) is implemented. This TODO covers the integration work to make it usable:

| Gap | Effort | Priority |
|-----|--------|----------|
| Server Router Wiring | 30 min | P0 - Nothing works without this |
| MCP Tools | 2-3 hours | P1 - Enables agent access |
| DSL Verbs | 3-4 hours | P2 - Enables DSL scripting |
| Taxonomy Visualization | 4-6 hours | P3 - UI enhancement |

---

## Phase 1: Server Router Wiring (30 min)

### 1.1 Export router from `rust/src/api/mod.rs`

Find the `mod.rs` file and add:

```rust
mod service_resource_routes;
pub use service_resource_routes::service_resource_router;
```

### 1.2 Merge into main.rs

**File:** `rust/crates/ob-poc-web/src/main.rs`

Find where other routers are merged (look for `.merge(` calls) and add:

```rust
use ob_poc::api::service_resource_router;

// In the router building section, add:
.merge(service_resource_router(pool.clone()))
```

### 1.3 Add startup logging

In the startup info section, add:

```rust
tracing::info!("Service Resource Pipeline endpoints:");
tracing::info!("  POST /api/cbu/:id/service-intents     - Create intent");
tracing::info!("  POST /api/cbu/:id/resource-discover   - Run discovery");
tracing::info!("  POST /api/cbu/:id/pipeline/full       - Full pipeline");
tracing::info!("  GET  /api/cbu/:id/readiness           - Service readiness");
tracing::info!("  GET  /api/srdefs                      - SRDEF registry");
```

### 1.4 Verify compilation

```bash
cargo build -p ob-poc-web
```

### 1.5 Test endpoints

```bash
# Start server
cargo run -p ob-poc-web

# Test SRDEF list
curl http://localhost:3000/api/srdefs

# Test service intent creation (use a valid CBU ID)
curl -X POST http://localhost:3000/api/cbu/{cbu_id}/service-intents \
  -H "Content-Type: application/json" \
  -d '{"product_id": "...", "service_id": "...", "options": {}}'
```

---

## Phase 2: MCP Tools (2-3 hours)

### 2.1 Add tool definitions

**File:** `rust/src/mcp/tools.rs`

Find the `get_tools()` function and add these tool definitions:

```rust
// === Service Resource Pipeline Tools ===

Tool {
    name: "service_intent_create".to_string(),
    description: "Create a service intent - declares that a CBU wants a product+service combination".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "cbu_id": {
                "type": "string",
                "format": "uuid",
                "description": "CBU UUID or name to look up"
            },
            "product_id": {
                "type": "string",
                "description": "Product UUID or name"
            },
            "service_id": {
                "type": "string",
                "description": "Service UUID or name"
            },
            "options": {
                "type": "object",
                "description": "Service configuration options (markets, SSI mode, etc.)",
                "additionalProperties": true
            }
        },
        "required": ["cbu_id", "product_id", "service_id"]
    }),
},

Tool {
    name: "service_intent_list".to_string(),
    description: "List all service intents for a CBU".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "cbu_id": {
                "type": "string",
                "format": "uuid",
                "description": "CBU UUID or name"
            }
        },
        "required": ["cbu_id"]
    }),
},

Tool {
    name: "service_discovery_run".to_string(),
    description: "Run resource discovery for a CBU - determines which SRDEFs are required based on service intents".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "cbu_id": {
                "type": "string",
                "format": "uuid",
                "description": "CBU UUID or name"
            }
        },
        "required": ["cbu_id"]
    }),
},

Tool {
    name: "service_attributes_gaps".to_string(),
    description: "Get attribute gaps for a CBU - shows which required attributes are missing values".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "cbu_id": {
                "type": "string",
                "format": "uuid",
                "description": "CBU UUID or name"
            }
        },
        "required": ["cbu_id"]
    }),
},

Tool {
    name: "service_attributes_set".to_string(),
    description: "Set an attribute value for a CBU".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "cbu_id": {
                "type": "string",
                "format": "uuid",
                "description": "CBU UUID or name"
            },
            "attr_id": {
                "type": "string",
                "description": "Attribute ID to set"
            },
            "value": {
                "description": "Value to set (type depends on attribute)"
            },
            "source": {
                "type": "string",
                "enum": ["manual", "document", "derived"],
                "default": "manual",
                "description": "Source of the value"
            },
            "evidence_refs": {
                "type": "array",
                "items": { "type": "string" },
                "description": "Document IDs supporting this value"
            }
        },
        "required": ["cbu_id", "attr_id", "value"]
    }),
},

Tool {
    name: "service_readiness_get".to_string(),
    description: "Get service readiness status for a CBU - shows which services are ready, blocked, or partial".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "cbu_id": {
                "type": "string",
                "format": "uuid",
                "description": "CBU UUID or name"
            }
        },
        "required": ["cbu_id"]
    }),
},

Tool {
    name: "service_readiness_recompute".to_string(),
    description: "Force recomputation of service readiness for a CBU".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "cbu_id": {
                "type": "string",
                "format": "uuid",
                "description": "CBU UUID or name"
            }
        },
        "required": ["cbu_id"]
    }),
},

Tool {
    name: "service_pipeline_run".to_string(),
    description: "Run the full service resource pipeline for a CBU: discovery → rollup → populate → provision → readiness".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "cbu_id": {
                "type": "string",
                "format": "uuid",
                "description": "CBU UUID or name"
            },
            "dry_run": {
                "type": "boolean",
                "default": false,
                "description": "If true, don't actually provision - just show what would happen"
            }
        },
        "required": ["cbu_id"]
    }),
},

Tool {
    name: "srdef_list".to_string(),
    description: "List all available Service Resource Definitions (SRDEFs)".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "domain": {
                "type": "string",
                "description": "Filter by domain (e.g., CUSTODY, CONNECTIVITY, IAM)"
            },
            "resource_type": {
                "type": "string",
                "description": "Filter by resource type (Account, Connectivity, Entitlement)"
            }
        }
    }),
},

Tool {
    name: "srdef_get".to_string(),
    description: "Get details of a specific SRDEF including its attribute requirements".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "srdef_id": {
                "type": "string",
                "description": "SRDEF ID (e.g., 'custody_securities', 'swift_messaging')"
            }
        },
        "required": ["srdef_id"]
    }),
},
```

### 2.2 Add tool handlers

**File:** `rust/src/mcp/handlers/core.rs` (or wherever tool handlers are implemented)

Add a new handler section for service resource tools:

```rust
use crate::service_resources::{
    ServiceResourcePipelineService,
    ResourceDiscoveryEngine,
    ReadinessEngine,
    SrdefRegistry,
};

// Handler for service_intent_create
async fn handle_service_intent_create(
    pool: &PgPool,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let cbu_id = resolve_cbu_id(pool, params.get("cbu_id")).await?;
    let product_id = resolve_product_id(pool, params.get("product_id")).await?;
    let service_id = resolve_service_id(pool, params.get("service_id")).await?;
    let options = params.get("options").cloned().unwrap_or(json!({}));
    
    let service = ServiceResourcePipelineService::new(pool.clone());
    let intent = service.create_service_intent(cbu_id, product_id, service_id, options).await
        .map_err(|e| e.to_string())?;
    
    Ok(json!({
        "success": true,
        "intent_id": intent.id,
        "message": format!("Created service intent for product {} service {}", 
                          intent.product_id, intent.service_id)
    }))
}

// Handler for service_intent_list
async fn handle_service_intent_list(
    pool: &PgPool,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let cbu_id = resolve_cbu_id(pool, params.get("cbu_id")).await?;
    
    let service = ServiceResourcePipelineService::new(pool.clone());
    let intents = service.list_service_intents(cbu_id).await
        .map_err(|e| e.to_string())?;
    
    Ok(json!({
        "success": true,
        "count": intents.len(),
        "intents": intents
    }))
}

// Handler for service_discovery_run
async fn handle_service_discovery_run(
    pool: &PgPool,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let cbu_id = resolve_cbu_id(pool, params.get("cbu_id")).await?;
    
    let service = ServiceResourcePipelineService::new(pool.clone());
    let result = service.run_discovery(cbu_id).await
        .map_err(|e| e.to_string())?;
    
    Ok(json!({
        "success": true,
        "srdefs_discovered": result.srdefs_discovered,
        "reasons": result.discovery_reasons
    }))
}

// Handler for service_attributes_gaps
async fn handle_service_attributes_gaps(
    pool: &PgPool,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let cbu_id = resolve_cbu_id(pool, params.get("cbu_id")).await?;
    
    let service = ServiceResourcePipelineService::new(pool.clone());
    let gaps = service.get_attribute_gaps(cbu_id).await
        .map_err(|e| e.to_string())?;
    
    Ok(json!({
        "success": true,
        "gap_count": gaps.len(),
        "gaps": gaps
    }))
}

// Handler for service_attributes_set
async fn handle_service_attributes_set(
    pool: &PgPool,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let cbu_id = resolve_cbu_id(pool, params.get("cbu_id")).await?;
    let attr_id = params.get("attr_id")
        .and_then(|v| v.as_str())
        .ok_or("attr_id required")?;
    let value = params.get("value")
        .ok_or("value required")?
        .clone();
    let source = params.get("source")
        .and_then(|v| v.as_str())
        .unwrap_or("manual");
    let evidence_refs: Vec<String> = params.get("evidence_refs")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();
    
    let service = ServiceResourcePipelineService::new(pool.clone());
    service.set_attribute_value(cbu_id, attr_id, value.clone(), source, evidence_refs).await
        .map_err(|e| e.to_string())?;
    
    Ok(json!({
        "success": true,
        "message": format!("Set attribute {} = {:?}", attr_id, value)
    }))
}

// Handler for service_readiness_get
async fn handle_service_readiness_get(
    pool: &PgPool,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let cbu_id = resolve_cbu_id(pool, params.get("cbu_id")).await?;
    
    let service = ServiceResourcePipelineService::new(pool.clone());
    let readiness = service.get_readiness(cbu_id).await
        .map_err(|e| e.to_string())?;
    
    let ready_count = readiness.iter().filter(|r| r.status == "ready").count();
    let blocked_count = readiness.iter().filter(|r| r.status == "blocked").count();
    
    Ok(json!({
        "success": true,
        "summary": {
            "ready": ready_count,
            "blocked": blocked_count,
            "partial": readiness.len() - ready_count - blocked_count
        },
        "services": readiness
    }))
}

// Handler for service_readiness_recompute
async fn handle_service_readiness_recompute(
    pool: &PgPool,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let cbu_id = resolve_cbu_id(pool, params.get("cbu_id")).await?;
    
    let service = ServiceResourcePipelineService::new(pool.clone());
    let result = service.recompute_readiness(cbu_id).await
        .map_err(|e| e.to_string())?;
    
    Ok(json!({
        "success": true,
        "recomputed": true,
        "readiness": result
    }))
}

// Handler for service_pipeline_run
async fn handle_service_pipeline_run(
    pool: &PgPool,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let cbu_id = resolve_cbu_id(pool, params.get("cbu_id")).await?;
    let dry_run = params.get("dry_run")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    
    let service = ServiceResourcePipelineService::new(pool.clone());
    let result = service.run_full_pipeline(cbu_id, dry_run).await
        .map_err(|e| e.to_string())?;
    
    Ok(json!({
        "success": true,
        "dry_run": dry_run,
        "pipeline_result": result
    }))
}

// Handler for srdef_list
async fn handle_srdef_list(
    pool: &PgPool,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let domain = params.get("domain").and_then(|v| v.as_str());
    let resource_type = params.get("resource_type").and_then(|v| v.as_str());
    
    let service = ServiceResourcePipelineService::new(pool.clone());
    let srdefs = service.list_srdefs(domain, resource_type).await
        .map_err(|e| e.to_string())?;
    
    Ok(json!({
        "success": true,
        "count": srdefs.len(),
        "srdefs": srdefs
    }))
}

// Handler for srdef_get
async fn handle_srdef_get(
    pool: &PgPool,
    params: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let srdef_id = params.get("srdef_id")
        .and_then(|v| v.as_str())
        .ok_or("srdef_id required")?;
    
    let service = ServiceResourcePipelineService::new(pool.clone());
    let srdef = service.get_srdef(srdef_id).await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("SRDEF not found: {}", srdef_id))?;
    
    Ok(json!({
        "success": true,
        "srdef": srdef
    }))
}
```

### 2.3 Wire handlers into tool dispatcher

In the main tool dispatch function (likely `handle_tool_call` or similar), add cases:

```rust
"service_intent_create" => handle_service_intent_create(pool, &params).await,
"service_intent_list" => handle_service_intent_list(pool, &params).await,
"service_discovery_run" => handle_service_discovery_run(pool, &params).await,
"service_attributes_gaps" => handle_service_attributes_gaps(pool, &params).await,
"service_attributes_set" => handle_service_attributes_set(pool, &params).await,
"service_readiness_get" => handle_service_readiness_get(pool, &params).await,
"service_readiness_recompute" => handle_service_readiness_recompute(pool, &params).await,
"service_pipeline_run" => handle_service_pipeline_run(pool, &params).await,
"srdef_list" => handle_srdef_list(pool, &params).await,
"srdef_get" => handle_srdef_get(pool, &params).await,
```

### 2.4 Add resolver helper functions

```rust
async fn resolve_cbu_id(pool: &PgPool, value: Option<&serde_json::Value>) -> Result<Uuid, String> {
    let value = value.ok_or("cbu_id required")?;
    
    // Try as UUID first
    if let Some(s) = value.as_str() {
        if let Ok(uuid) = Uuid::parse_str(s) {
            return Ok(uuid);
        }
        // Try as name lookup
        let cbu_id: Option<Uuid> = sqlx::query_scalar!(
            "SELECT cbu_id FROM cbus WHERE name ILIKE $1",
            s
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
        
        return cbu_id.ok_or_else(|| format!("CBU not found: {}", s));
    }
    
    Err("cbu_id must be string (UUID or name)".to_string())
}

async fn resolve_product_id(pool: &PgPool, value: Option<&serde_json::Value>) -> Result<Uuid, String> {
    let value = value.ok_or("product_id required")?;
    
    if let Some(s) = value.as_str() {
        if let Ok(uuid) = Uuid::parse_str(s) {
            return Ok(uuid);
        }
        let product_id: Option<Uuid> = sqlx::query_scalar!(
            "SELECT product_id FROM products WHERE name ILIKE $1",
            s
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
        
        return product_id.ok_or_else(|| format!("Product not found: {}", s));
    }
    
    Err("product_id must be string (UUID or name)".to_string())
}

async fn resolve_service_id(pool: &PgPool, value: Option<&serde_json::Value>) -> Result<Uuid, String> {
    let value = value.ok_or("service_id required")?;
    
    if let Some(s) = value.as_str() {
        if let Ok(uuid) = Uuid::parse_str(s) {
            return Ok(uuid);
        }
        let service_id: Option<Uuid> = sqlx::query_scalar!(
            "SELECT service_id FROM services WHERE name ILIKE $1",
            s
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;
        
        return service_id.ok_or_else(|| format!("Service not found: {}", s));
    }
    
    Err("service_id must be string (UUID or name)".to_string())
}
```

---

## Phase 3: DSL Verbs (3-4 hours)

### 3.1 Create verb configuration

**File:** `rust/config/verbs/service-pipeline.yaml` (NEW FILE)

```yaml
# Service Resource Pipeline DSL Verbs
# 
# These verbs orchestrate the Product → Service → Resource lifecycle:
# 1. service-intent.create - Declare what CBU wants
# 2. discovery.run - Determine required SRDEFs
# 3. attributes.rollup - Merge attribute requirements
# 4. attributes.populate - Fill attribute values
# 5. provisioning.run - Create resources
# 6. readiness.compute - Check "good to transact" status

domains:
  service-intent:
    description: "Service intent management - declares what products/services a CBU wants"
    
    verbs:
      create:
        description: "Create a service intent for a CBU"
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
            description: "CBU to create intent for"
            lookup:
              table: cbus
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: product-id
            type: uuid
            required: true
            description: "Product being subscribed to"
            lookup:
              table: products
              schema: ob-poc
              search_key: name
              primary_key: product_id
          - name: service-id
            type: uuid
            required: true
            description: "Service being configured"
            lookup:
              table: services
              schema: ob-poc
              search_key: name
              primary_key: service_id
          - name: options
            type: json
            required: false
            description: "Service configuration options"
        returns:
          type: uuid
          name: intent_id
          capture: true

      list:
        description: "List all service intents for a CBU"
        metadata:
          tier: diagnostics
          source_of_truth: operational
          scope: cbu
          noun: service_intent
        behavior: plugin
        handler: ServiceIntentListOp
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: array
          name: intents

      supersede:
        description: "Supersede an existing service intent with new options"
        metadata:
          tier: intent
          source_of_truth: operational
          scope: cbu
          noun: service_intent
        behavior: plugin
        handler: ServiceIntentSupersedeOp
        args:
          - name: intent-id
            type: uuid
            required: true
          - name: options
            type: json
            required: true
        returns:
          type: uuid
          name: new_intent_id

  discovery:
    description: "Resource discovery operations"
    
    verbs:
      run:
        description: "Run resource discovery - determines required SRDEFs from service intents"
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
            lookup:
              table: cbus
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: record
          name: discovery_result
          fields:
            - srdefs_discovered
            - discovery_reasons

      explain:
        description: "Explain why specific SRDEFs were discovered"
        metadata:
          tier: diagnostics
          source_of_truth: operational
          scope: cbu
          noun: discovery
        behavior: plugin
        handler: DiscoveryExplainOp
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: srdef-id
            type: string
            required: false
            description: "Specific SRDEF to explain (or all if omitted)"
        returns:
          type: array
          name: explanations

  attributes:
    description: "Attribute requirement and value management"
    
    verbs:
      rollup:
        description: "Roll up attribute requirements from discovered SRDEFs"
        metadata:
          tier: composite
          source_of_truth: operational
          scope: cbu
          noun: attribute_requirements
        behavior: plugin
        handler: AttributeRollupOp
        args:
          - name: cbu-id
            type: uuid
            required: true
        returns:
          type: record
          name: rollup_result

      populate:
        description: "Auto-populate attribute values from available sources"
        metadata:
          tier: composite
          source_of_truth: operational
          scope: cbu
          noun: attribute_values
        behavior: plugin
        handler: AttributePopulateOp
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: sources
            type: array
            required: false
            description: "Sources to try: entity, cbu, document, derived"
        returns:
          type: record
          name: population_result

      gaps:
        description: "Show attribute gaps - required values that are missing"
        metadata:
          tier: diagnostics
          source_of_truth: operational
          scope: cbu
          noun: attribute_gaps
        behavior: plugin
        handler: AttributeGapsOp
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: srdef-id
            type: string
            required: false
            description: "Filter to specific SRDEF"
        returns:
          type: array
          name: gaps

      set:
        description: "Set an attribute value manually"
        metadata:
          tier: intent
          source_of_truth: operational
          scope: cbu
          noun: attribute_value
        behavior: plugin
        handler: AttributeSetOp
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: attr-id
            type: string
            required: true
          - name: value
            type: any
            required: true
          - name: evidence
            type: array
            required: false
            description: "Document references supporting this value"
        returns:
          type: boolean
          name: success

  provisioning:
    description: "Resource provisioning operations"
    
    verbs:
      run:
        description: "Run provisioning orchestrator - creates resources for ready SRDEFs"
        metadata:
          tier: composite
          source_of_truth: operational
          scope: cbu
          noun: provisioning
        behavior: plugin
        handler: ProvisioningRunOp
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: dry-run
            type: boolean
            required: false
            default: false
        returns:
          type: record
          name: provisioning_result

      status:
        description: "Check provisioning request status"
        metadata:
          tier: diagnostics
          source_of_truth: operational
          scope: cbu
          noun: provisioning_status
        behavior: plugin
        handler: ProvisioningStatusOp
        args:
          - name: request-id
            type: uuid
            required: true
        returns:
          type: record
          name: request_status

  readiness:
    description: "Service readiness operations"
    
    verbs:
      compute:
        description: "Compute service readiness - determines 'good to transact' status"
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
        description: "Explain blocking reasons for a service"
        metadata:
          tier: diagnostics
          source_of_truth: operational
          scope: cbu
          noun: blocking_reasons
        behavior: plugin
        handler: ReadinessExplainOp
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: service-id
            type: uuid
            required: false
            description: "Specific service (or all if omitted)"
        returns:
          type: array
          name: blocking_reasons

  pipeline:
    description: "Full pipeline orchestration"
    
    verbs:
      full:
        description: "Run the entire pipeline: discovery → rollup → populate → provision → readiness"
        metadata:
          tier: composite
          source_of_truth: operational
          scope: cbu
          noun: pipeline
        behavior: plugin
        handler: PipelineFullOp
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: dry-run
            type: boolean
            required: false
            default: false
        returns:
          type: record
          name: pipeline_result
```

### 3.2 Create plugin handlers

**File:** `rust/src/dsl_v2/custom_ops/service_pipeline.rs` (NEW FILE)

```rust
//! Plugin handlers for Service Resource Pipeline DSL verbs

use crate::dsl_v2::{DslValue, VerbContext, VerbResult};
use crate::service_resources::ServiceResourcePipelineService;
use async_trait::async_trait;
use uuid::Uuid;

/// Handler for service-intent.create
pub struct ServiceIntentCreateOp;

#[async_trait]
impl crate::dsl_v2::PluginOp for ServiceIntentCreateOp {
    async fn execute(&self, ctx: &VerbContext) -> VerbResult {
        let cbu_id: Uuid = ctx.get_arg("cbu-id")?;
        let product_id: Uuid = ctx.get_arg("product-id")?;
        let service_id: Uuid = ctx.get_arg("service-id")?;
        let options = ctx.get_arg_or("options", serde_json::json!({}));
        
        let service = ServiceResourcePipelineService::new(ctx.pool().clone());
        let intent = service.create_service_intent(cbu_id, product_id, service_id, options).await?;
        
        Ok(DslValue::Uuid(intent.id))
    }
}

/// Handler for service-intent.list
pub struct ServiceIntentListOp;

#[async_trait]
impl crate::dsl_v2::PluginOp for ServiceIntentListOp {
    async fn execute(&self, ctx: &VerbContext) -> VerbResult {
        let cbu_id: Uuid = ctx.get_arg("cbu-id")?;
        
        let service = ServiceResourcePipelineService::new(ctx.pool().clone());
        let intents = service.list_service_intents(cbu_id).await?;
        
        Ok(DslValue::Array(intents.into_iter().map(|i| DslValue::from(i)).collect()))
    }
}

/// Handler for discovery.run
pub struct DiscoveryRunOp;

#[async_trait]
impl crate::dsl_v2::PluginOp for DiscoveryRunOp {
    async fn execute(&self, ctx: &VerbContext) -> VerbResult {
        let cbu_id: Uuid = ctx.get_arg("cbu-id")?;
        
        let service = ServiceResourcePipelineService::new(ctx.pool().clone());
        let result = service.run_discovery(cbu_id).await?;
        
        Ok(DslValue::Record(serde_json::to_value(result)?))
    }
}

/// Handler for attributes.gaps
pub struct AttributeGapsOp;

#[async_trait]
impl crate::dsl_v2::PluginOp for AttributeGapsOp {
    async fn execute(&self, ctx: &VerbContext) -> VerbResult {
        let cbu_id: Uuid = ctx.get_arg("cbu-id")?;
        let srdef_id: Option<String> = ctx.get_arg_opt("srdef-id");
        
        let service = ServiceResourcePipelineService::new(ctx.pool().clone());
        let gaps = service.get_attribute_gaps(cbu_id).await?;
        
        // Filter by SRDEF if specified
        let filtered = if let Some(ref srdef) = srdef_id {
            gaps.into_iter()
                .filter(|g| g.required_by_srdefs.contains(srdef))
                .collect()
        } else {
            gaps
        };
        
        Ok(DslValue::Array(filtered.into_iter().map(|g| DslValue::from(g)).collect()))
    }
}

/// Handler for attributes.set
pub struct AttributeSetOp;

#[async_trait]
impl crate::dsl_v2::PluginOp for AttributeSetOp {
    async fn execute(&self, ctx: &VerbContext) -> VerbResult {
        let cbu_id: Uuid = ctx.get_arg("cbu-id")?;
        let attr_id: String = ctx.get_arg("attr-id")?;
        let value: serde_json::Value = ctx.get_arg("value")?;
        let evidence: Vec<String> = ctx.get_arg_or("evidence", vec![]);
        
        let service = ServiceResourcePipelineService::new(ctx.pool().clone());
        service.set_attribute_value(cbu_id, &attr_id, value, "manual", evidence).await?;
        
        Ok(DslValue::Bool(true))
    }
}

/// Handler for readiness.compute
pub struct ReadinessComputeOp;

#[async_trait]
impl crate::dsl_v2::PluginOp for ReadinessComputeOp {
    async fn execute(&self, ctx: &VerbContext) -> VerbResult {
        let cbu_id: Uuid = ctx.get_arg("cbu-id")?;
        
        let service = ServiceResourcePipelineService::new(ctx.pool().clone());
        let result = service.recompute_readiness(cbu_id).await?;
        
        Ok(DslValue::Record(serde_json::to_value(result)?))
    }
}

/// Handler for readiness.explain
pub struct ReadinessExplainOp;

#[async_trait]
impl crate::dsl_v2::PluginOp for ReadinessExplainOp {
    async fn execute(&self, ctx: &VerbContext) -> VerbResult {
        let cbu_id: Uuid = ctx.get_arg("cbu-id")?;
        let service_id: Option<Uuid> = ctx.get_arg_opt("service-id");
        
        let service = ServiceResourcePipelineService::new(ctx.pool().clone());
        let readiness = service.get_readiness(cbu_id).await?;
        
        let blocking: Vec<_> = readiness.into_iter()
            .filter(|r| service_id.map_or(true, |sid| r.service_id == sid))
            .filter(|r| r.status != "ready")
            .flat_map(|r| r.blocking_reasons)
            .collect();
        
        Ok(DslValue::Array(blocking.into_iter().map(|b| DslValue::from(b)).collect()))
    }
}

/// Handler for pipeline.full
pub struct PipelineFullOp;

#[async_trait]
impl crate::dsl_v2::PluginOp for PipelineFullOp {
    async fn execute(&self, ctx: &VerbContext) -> VerbResult {
        let cbu_id: Uuid = ctx.get_arg("cbu-id")?;
        let dry_run: bool = ctx.get_arg_or("dry-run", false);
        
        let service = ServiceResourcePipelineService::new(ctx.pool().clone());
        let result = service.run_full_pipeline(cbu_id, dry_run).await?;
        
        Ok(DslValue::Record(serde_json::to_value(result)?))
    }
}

/// Handler for provisioning.run
pub struct ProvisioningRunOp;

#[async_trait]
impl crate::dsl_v2::PluginOp for ProvisioningRunOp {
    async fn execute(&self, ctx: &VerbContext) -> VerbResult {
        let cbu_id: Uuid = ctx.get_arg("cbu-id")?;
        let dry_run: bool = ctx.get_arg_or("dry-run", false);
        
        let service = ServiceResourcePipelineService::new(ctx.pool().clone());
        let result = service.run_provisioning(cbu_id, dry_run).await?;
        
        Ok(DslValue::Record(serde_json::to_value(result)?))
    }
}
```

### 3.3 Register handlers

**File:** `rust/src/dsl_v2/custom_ops/mod.rs`

Add:
```rust
mod service_pipeline;
pub use service_pipeline::*;
```

**File:** `rust/src/dsl_v2/verb_registry.rs` (or wherever handlers are registered)

Add registration:
```rust
use crate::dsl_v2::custom_ops::{
    ServiceIntentCreateOp, ServiceIntentListOp,
    DiscoveryRunOp, AttributeGapsOp, AttributeSetOp,
    ReadinessComputeOp, ReadinessExplainOp,
    PipelineFullOp, ProvisioningRunOp,
};

// In registration function:
registry.register_handler("ServiceIntentCreateOp", Box::new(ServiceIntentCreateOp));
registry.register_handler("ServiceIntentListOp", Box::new(ServiceIntentListOp));
registry.register_handler("DiscoveryRunOp", Box::new(DiscoveryRunOp));
registry.register_handler("AttributeGapsOp", Box::new(AttributeGapsOp));
registry.register_handler("AttributeSetOp", Box::new(AttributeSetOp));
registry.register_handler("ReadinessComputeOp", Box::new(ReadinessComputeOp));
registry.register_handler("ReadinessExplainOp", Box::new(ReadinessExplainOp));
registry.register_handler("PipelineFullOp", Box::new(PipelineFullOp));
registry.register_handler("ProvisioningRunOp", Box::new(ProvisioningRunOp));
```

---

## Phase 4: Taxonomy Visualization (4-6 hours) [OPTIONAL]

### 4.1 Add taxonomy API routes

**File:** `rust/src/api/taxonomy_product_routes.rs` (NEW FILE)

```rust
//! Product → Service → Resource taxonomy API

use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use sqlx::PgPool;
use uuid::Uuid;

pub fn taxonomy_product_router(pool: PgPool) -> Router {
    Router::new()
        .route("/taxonomy/products", get(list_products))
        .route("/taxonomy/products/:id/services", get(list_product_services))
        .route("/taxonomy/services/:id/resources", get(list_service_resources))
        .route("/taxonomy/srdefs/:id/dependencies", get(get_srdef_dependencies))
        .with_state(pool)
}

async fn list_products(
    State(pool): State<PgPool>,
) -> Json<Vec<ProductSummary>> {
    let products = sqlx::query_as!(
        ProductSummary,
        r#"
        SELECT p.product_id, p.name, p.description,
               COUNT(DISTINCT ps.service_id) as service_count
        FROM products p
        LEFT JOIN product_services ps ON p.product_id = ps.product_id
        GROUP BY p.product_id, p.name, p.description
        ORDER BY p.name
        "#
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();
    
    Json(products)
}

async fn list_product_services(
    State(pool): State<PgPool>,
    Path(product_id): Path<Uuid>,
) -> Json<Vec<ServiceSummary>> {
    let services = sqlx::query_as!(
        ServiceSummary,
        r#"
        SELECT s.service_id, s.name, s.description,
               COUNT(DISTINCT src.resource_type_id) as resource_count
        FROM services s
        JOIN product_services ps ON s.service_id = ps.service_id
        LEFT JOIN service_resource_capabilities src ON s.service_id = src.service_id
        WHERE ps.product_id = $1
        GROUP BY s.service_id, s.name, s.description
        ORDER BY s.name
        "#,
        product_id
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();
    
    Json(services)
}

async fn list_service_resources(
    State(pool): State<PgPool>,
    Path(service_id): Path<Uuid>,
) -> Json<Vec<ResourceTypeSummary>> {
    let resources = sqlx::query_as!(
        ResourceTypeSummary,
        r#"
        SELECT srt.resource_type_id, srt.code, srt.name, srt.resource_kind,
               srt.srdef_id, srt.provisioning_strategy,
               COALESCE(srt.depends_on, '{}') as dependencies
        FROM service_resource_types srt
        JOIN service_resource_capabilities src ON srt.resource_type_id = src.resource_type_id
        WHERE src.service_id = $1
        ORDER BY srt.name
        "#,
        service_id
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default();
    
    Json(resources)
}

async fn get_srdef_dependencies(
    State(pool): State<PgPool>,
    Path(srdef_id): Path<String>,
) -> Json<DependencyTree> {
    // Build dependency tree recursively
    let tree = build_dependency_tree(&pool, &srdef_id).await;
    Json(tree)
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct ProductSummary {
    product_id: Uuid,
    name: String,
    description: Option<String>,
    service_count: i64,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct ServiceSummary {
    service_id: Uuid,
    name: String,
    description: Option<String>,
    resource_count: i64,
}

#[derive(serde::Serialize, sqlx::FromRow)]
struct ResourceTypeSummary {
    resource_type_id: Uuid,
    code: String,
    name: String,
    resource_kind: String,
    srdef_id: Option<String>,
    provisioning_strategy: Option<String>,
    dependencies: Vec<String>,
}

#[derive(serde::Serialize)]
struct DependencyTree {
    srdef_id: String,
    name: String,
    depends_on: Vec<DependencyTree>,
}

async fn build_dependency_tree(pool: &PgPool, srdef_id: &str) -> DependencyTree {
    let srdef = sqlx::query!(
        "SELECT code, name, depends_on FROM service_resource_types WHERE srdef_id = $1",
        srdef_id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();
    
    let (name, deps) = match srdef {
        Some(s) => (s.name, s.depends_on.unwrap_or_default()),
        None => (srdef_id.to_string(), vec![]),
    };
    
    let mut children = vec![];
    for dep in deps {
        children.push(Box::pin(build_dependency_tree(pool, &dep)).await);
    }
    
    DependencyTree {
        srdef_id: srdef_id.to_string(),
        name,
        depends_on: children,
    }
}
```

### 4.2 Wire taxonomy routes

**File:** `rust/crates/ob-poc-web/src/main.rs`

```rust
use ob_poc::api::taxonomy_product_router;

// Add to router merging:
.merge(taxonomy_product_router(pool.clone()))
```

### 4.3 UI Implementation

This is optional and can be deferred. The egui/WASM panel work would require:

1. New panel: `product_taxonomy.rs`
2. Tree view component for Product → Service → Resource navigation
3. Integration with existing graph visualization for dependency display

---

## Testing Checklist

After implementing each phase:

### Phase 1: Server Wiring
- [ ] `curl http://localhost:3000/api/srdefs` returns SRDEF list
- [ ] `curl http://localhost:3000/api/cbu/{id}/service-intents` works
- [ ] Server startup logs show service resource endpoints

### Phase 2: MCP Tools
- [ ] `service_readiness_get` returns readiness via MCP
- [ ] `srdef_list` returns SRDEFs via MCP
- [ ] `service_pipeline_run` executes full pipeline via MCP

### Phase 3: DSL Verbs
- [ ] `discovery.run cbu-id=...` executes via CLI
- [ ] `readiness.compute cbu-id=...` executes via CLI
- [ ] `attributes.gaps cbu-id=...` shows missing attributes

### Phase 4: Taxonomy Viz (if implemented)
- [ ] `/api/taxonomy/products` returns product list
- [ ] UI shows navigable hierarchy

---

## Dependencies

Ensure these are in `Cargo.toml`:
```toml
# Already present but verify:
async-trait = "0.1"
uuid = { version = "1", features = ["serde", "v4"] }
```

---

## Total Effort

| Phase | Time | Priority |
|-------|------|----------|
| Phase 1: Server Wiring | 30 min | P0 |
| Phase 2: MCP Tools | 2-3 hours | P1 |
| Phase 3: DSL Verbs | 3-4 hours | P2 |
| Phase 4: Taxonomy Viz | 4-6 hours | P3 (optional) |
| **Total** | **10-14 hours** | |

---

## Files Summary

### New Files
```
rust/config/verbs/service-pipeline.yaml
rust/src/dsl_v2/custom_ops/service_pipeline.rs
rust/src/api/taxonomy_product_routes.rs (optional)
```

### Modified Files
```
rust/src/api/mod.rs                     - Export service_resource_router
rust/crates/ob-poc-web/src/main.rs      - Merge router
rust/src/mcp/tools.rs                   - Add 10 tool definitions
rust/src/mcp/handlers/core.rs           - Add tool handlers
rust/src/dsl_v2/custom_ops/mod.rs       - Export service_pipeline
rust/src/dsl_v2/verb_registry.rs        - Register handlers
```
