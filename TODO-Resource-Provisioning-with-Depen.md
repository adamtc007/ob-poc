# TODO: Resource Provisioning with Dependency Graph Integration

## Context

Building on the existing ob-poc platform to add **Terraform-like resource provisioning** for CBU onboarding. The DSL already has composite search keys, topological sorting with lifecycle awareness, and basic service-resource provisioning. This TODO adds **resource dependency graph support** that integrates with the existing DSL compiler.

## Current State (What Already Exists)

### DSL Infrastructure ✓
- `topo_sort.rs` - Topological sorting with Kahn's algorithm, cycle detection, lifecycle awareness
- `execution_plan.rs` - Execution plan compiler with `compile_with_planning()`, synthetic step injection
- `config/types.rs` - `SearchKeyConfig` with s-expression parsing, `CompositeSearchKey`, resolution tiers
- `binding_context.rs` - Binding tracking for `@symbol` references

### Service Resource Domain ✓
Verb config: `config/verbs/service-resource.yaml`
Handlers implemented in `custom_ops/mod.rs`:
- `service-resource.provision` → `ResourceCreateOp` (idempotent insert into `cbu_resource_instances`)
- `service-resource.set-attr` → `ResourceSetAttrOp`
- `service-resource.activate` → `ResourceActivateOp`
- `service-resource.suspend` → `ResourceSuspendOp`
- `service-resource.decommission` → `ResourceDecommissionOp`
- `service-resource.validate-attrs` → `ResourceValidateAttrsOp`

### Database Schema ✓
```sql
-- Product/Service/Resource Taxonomy
products                      -- Product catalog (CUSTODY_FULL, etc.)
services                      -- Generic services (safekeeping, settlement, etc.)
product_services              -- Product → Service links (many-to-many)
service_resource_types        -- Resource type definitions (custody_account, swift_parser, etc.)
service_resource_capabilities -- Service → Resource links with options
resource_attribute_requirements -- Attributes needed per resource type

-- Instances
cbu_resource_instances        -- Provisioned resources for CBUs

-- Onboarding Workflow
onboarding_requests           -- Workflow state machine
onboarding_products           -- Products selected for onboarding
```

---

## Phase 1: Resource Dependency Schema

### 1.1 Add `resource_dependencies` Table

```sql
-- Track which resource types depend on other resource types
CREATE TABLE "ob-poc".resource_dependencies (
    dependency_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    resource_type_id UUID NOT NULL REFERENCES "ob-poc".service_resource_types(resource_id),
    depends_on_type_id UUID NOT NULL REFERENCES "ob-poc".service_resource_types(resource_id),
    
    -- Dependency metadata
    dependency_type VARCHAR(20) DEFAULT 'required' 
        CHECK (dependency_type IN ('required', 'optional', 'conditional')),
    
    -- Which argument receives the dependency's URL
    inject_arg VARCHAR(100) NOT NULL,
    
    -- For conditional dependencies
    condition_expression TEXT,  -- e.g., "product.has_feature('multi_currency')"
    
    -- Ordering hint for same-level dependencies
    priority INTEGER DEFAULT 100,
    
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    
    CONSTRAINT no_self_dependency CHECK (resource_type_id != depends_on_type_id),
    UNIQUE(resource_type_id, depends_on_type_id)
);

CREATE INDEX idx_resource_deps_type ON "ob-poc".resource_dependencies(resource_type_id);
CREATE INDEX idx_resource_deps_on ON "ob-poc".resource_dependencies(depends_on_type_id);

COMMENT ON TABLE "ob-poc".resource_dependencies IS 
'Resource type dependencies for onboarding. E.g., custody_account depends on cash_account.
The inject_arg specifies which provisioning argument receives the dependency URL.';
```

### 1.2 Seed Resource Dependencies

```sql
-- Example dependencies for custody product
INSERT INTO "ob-poc".resource_dependencies 
(resource_type_id, depends_on_type_id, dependency_type, inject_arg, priority)
SELECT 
    (SELECT resource_id FROM "ob-poc".service_resource_types WHERE resource_code = 'CUSTODY_ACCOUNT'),
    (SELECT resource_id FROM "ob-poc".service_resource_types WHERE resource_code = 'CASH_ACCOUNT'),
    'required',
    'cash-account-url',
    100;

INSERT INTO "ob-poc".resource_dependencies 
(resource_type_id, depends_on_type_id, dependency_type, inject_arg, priority)
SELECT 
    (SELECT resource_id FROM "ob-poc".service_resource_types WHERE resource_code = 'SWIFT_PARSER'),
    (SELECT resource_id FROM "ob-poc".service_resource_types WHERE resource_code = 'CUSTODY_ACCOUNT'),
    'required',
    'custody-account-url',
    100;

-- settlement_instructions, reporting_subscription also depend on custody_account
```

### 1.3 Add `resource_instance_dependencies` for Runtime Tracking

```sql
-- Track actual instance-level dependencies (post-provisioning)
CREATE TABLE "ob-poc".resource_instance_dependencies (
    instance_id UUID NOT NULL REFERENCES "ob-poc".cbu_resource_instances(instance_id),
    depends_on_instance_id UUID NOT NULL REFERENCES "ob-poc".cbu_resource_instances(instance_id),
    dependency_type VARCHAR(20) DEFAULT 'required',
    created_at TIMESTAMPTZ DEFAULT now(),
    PRIMARY KEY (instance_id, depends_on_instance_id)
);
```

---

## Phase 2: DSL Integration with Dependency Graph

### 2.1 Extend `service-resource.provision` Verb

Update `config/verbs/service-resource.yaml`:

```yaml
provision:
  description: Provision a service resource instance for a CBU
  behavior: plugin
  handler: resource_instance_create
  args:
    # ... existing args ...
    
    # NEW: Dependency references (list of @symbols or URLs)
    - name: depends-on
      type: string_list
      required: false
      description: |
        URLs or @symbol references of resources this instance depends on.
        Detected by the DSL compiler for topological ordering.
    
    # NEW: Specific dependency injections (from resource_dependencies config)
    - name: cash-account-url
      type: string
      required: false
      description: URL of cash account (auto-injected for custody_account)
    
    - name: custody-account-url
      type: string
      required: false
      description: URL of custody account (auto-injected for dependent resources)
  
  # Dataflow for compiler
  consumes:
    - arg: cbu-id
      type: cbu
    - arg: depends-on
      type: resource_instance
      required: false
  
  produces:
    type: resource_instance
    resolved: false
  
  returns:
    type: record
    name: resource
    # { instance_id, instance_url, status }
```

### 2.2 Update `ResourceCreateOp` Handler

In `custom_ops/mod.rs`, extend to:
1. Accept `depends-on` list of URLs/symbols
2. Record dependencies in `resource_instance_dependencies`
3. Extract specific injection args (`cash-account-url`, etc.)

```rust
// In ResourceCreateOp::execute()

// Extract depends-on list (URLs or resolved @symbols)
let depends_on: Vec<String> = verb_call
    .arguments
    .iter()
    .find(|a| a.key == "depends-on")
    .map(|a| {
        a.value.as_list()
            .map(|items| items.iter()
                .filter_map(|item| {
                    if let Some(name) = item.as_symbol() {
                        // Resolve @symbol to URL via context
                        ctx.resolve_url(name)
                    } else {
                        item.as_string().map(|s| s.to_string())
                    }
                })
                .collect())
            .unwrap_or_default()
    })
    .unwrap_or_default();

// After creating instance, record dependencies
for dep_url in &depends_on {
    let dep_instance_id: Option<Uuid> = sqlx::query_scalar(
        r#"SELECT instance_id FROM "ob-poc".cbu_resource_instances WHERE instance_url = $1"#
    )
    .bind(dep_url)
    .fetch_optional(pool)
    .await?;
    
    if let Some(dep_id) = dep_instance_id {
        sqlx::query(
            r#"INSERT INTO "ob-poc".resource_instance_dependencies 
               (instance_id, depends_on_instance_id) 
               VALUES ($1, $2) ON CONFLICT DO NOTHING"#
        )
        .bind(result_id)
        .bind(dep_id)
        .execute(pool)
        .await?;
    }
}
```

### 2.3 Topological Sort Already Handles Lists ✓ VERIFIED

The existing `topo_sort.rs` already handles `@symbol` references inside list arguments. 

**Verified in topo_sort.rs lines 573-576:**
```rust
AstNode::List { items, .. } => {
    for item in items {
        collect_symbol_refs(item, binding_to_stmt, _executed_context, current_idx, deps);
    }
}
```

This means `:depends-on [@foo @bar]` will correctly create dependency edges to statements producing `@foo` and `@bar`. **No changes needed.**

---

## Phase 3: Onboarding Workflow Verbs

### 3.1 Add `onboarding` Domain

Create `config/verbs/onboarding.yaml`:

```yaml
domains:
  onboarding:
    description: "CBU onboarding workflow orchestration"
    verbs:
      # =====================================================================
      # PLANNING
      # =====================================================================
      
      plan:
        description: |
          Create onboarding plan by expanding products to resources.
          Generates service-resource.provision statements with dependency ordering.
        behavior: plugin
        handler: onboarding_plan
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: products
            type: string_list
            required: true
            description: List of product codes to onboard
        produces:
          type: onboarding_plan
        returns:
          type: uuid
          name: plan_id
          capture: true
      
      show-plan:
        description: Display generated DSL statements and dependency graph
        behavior: plugin
        handler: onboarding_show_plan
        args:
          - name: plan-id
            type: uuid
            required: true
        returns:
          type: record
          # { dsl: "...", resources: [...], stages: [[0,1],[2]], dependency_graph: {...} }
      
      override-attribute:
        description: Override an attribute value in the plan before execution
        behavior: plugin
        handler: onboarding_override_attr
        args:
          - name: plan-id
            type: uuid
            required: true
          - name: resource
            type: string
            required: true
            description: Resource type code (e.g., "CUSTODY_ACCOUNT")
          - name: attribute
            type: string
            required: true
          - name: value
            type: string
            required: true
      
      validate-plan:
        description: Validate plan for completeness and capability coverage
        behavior: plugin
        handler: onboarding_validate_plan
        args:
          - name: plan-id
            type: uuid
            required: true
        returns:
          type: record
          # { valid: true/false, errors: [...], warnings: [...] }
      
      # =====================================================================
      # EXECUTION
      # =====================================================================
      
      execute:
        description: |
          Compile and execute the plan's DSL statements.
          Uses standard DSL compiler for dependency ordering.
        behavior: plugin
        handler: onboarding_execute
        args:
          - name: plan-id
            type: uuid
            required: true
          - name: parallel
            type: boolean
            required: false
            default: true
            description: Execute stages in parallel where possible
        produces:
          type: onboarding_execution
        returns:
          type: uuid
          name: execution_id
          capture: true
      
      status:
        description: Check execution progress
        behavior: plugin
        handler: onboarding_status
        args:
          - name: execution-id
            type: uuid
            required: true
        returns:
          type: record
          # { status, completed, total, current_stage, failed_tasks: [...] }
      
      await:
        description: Wait for execution to complete (with timeout)
        behavior: plugin
        handler: onboarding_await
        args:
          - name: execution-id
            type: uuid
            required: true
          - name: timeout
            type: integer
            required: false
            default: 300
            description: Timeout in seconds
        returns:
          type: record
      
      get-urls:
        description: Retrieve resource URLs after successful execution
        behavior: plugin
        handler: onboarding_get_urls
        args:
          - name: execution-id
            type: uuid
            required: true
        returns:
          type: record
          # { custody_account: "urn:...", cash_account: "urn:...", ... }
      
      # =====================================================================
      # SIMPLE PATH
      # =====================================================================
      
      ensure:
        description: |
          Idempotent onboarding - plan, compile, execute in one operation.
          Checks for existing resources before provisioning new ones.
        behavior: plugin
        handler: onboarding_ensure
        args:
          - name: cbu-id
            type: uuid
            required: true
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
          - name: products
            type: string_list
            required: true
          - name: wait
            type: boolean
            required: false
            default: true
        returns:
          type: record
          # { status: "complete", resources: [...], urls: {...} }
```

### 3.2 Implement Onboarding Handlers

Create `custom_ops/onboarding.rs`:

```rust
//! Onboarding workflow operations
//!
//! These operations orchestrate the onboarding of CBUs to products by:
//! 1. Expanding products → services → resources
//! 2. Building dependency graph from resource_dependencies table
//! 3. Generating DSL statements with @symbol dependencies
//! 4. Delegating to standard DSL compiler for ordering
//! 5. Executing stages (optionally in parallel)

pub struct OnboardingPlanOp;

#[async_trait]
impl CustomOperation for OnboardingPlanOp {
    fn domain(&self) -> &'static str { "onboarding" }
    fn verb(&self) -> &'static str { "plan" }
    fn rationale(&self) -> &'static str {
        "Requires product→service→resource expansion and dependency graph traversal"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let products = extract_string_list(verb_call, "products")?;
        
        // 1. Expand products to resource types
        let resources = expand_products_to_resources(pool, &products).await?;
        
        // 2. Build dependency graph
        let dep_graph = build_resource_dependency_graph(pool, &resources).await?;
        
        // 3. Generate DSL statements
        let dsl = generate_provisioning_dsl(&cbu_id, &resources, &dep_graph);
        
        // 4. Store plan
        let plan_id = store_onboarding_plan(pool, cbu_id, &products, &dsl, &dep_graph).await?;
        
        ctx.bind("plan", plan_id);
        Ok(ExecutionResult::Uuid(plan_id))
    }
}

/// Expand products → services → resources
async fn expand_products_to_resources(
    pool: &PgPool,
    products: &[String],
) -> Result<Vec<ResourceToProvision>> {
    let mut resources = Vec::new();
    
    for product_code in products {
        // Get services for this product
        let services: Vec<(Uuid, String, bool)> = sqlx::query_as(
            r#"SELECT s.service_id, s.name, ps.is_mandatory
               FROM "ob-poc".product_services ps
               JOIN "ob-poc".services s ON s.service_id = ps.service_id
               JOIN "ob-poc".products p ON p.product_id = ps.product_id
               WHERE p.product_code = $1 AND s.is_active = true"#
        )
        .bind(product_code)
        .fetch_all(pool)
        .await?;
        
        for (service_id, service_name, is_mandatory) in services {
            // Get resources that provide this service
            let service_resources: Vec<(Uuid, String, String)> = sqlx::query_as(
                r#"SELECT srt.resource_id, srt.resource_code, srt.name
                   FROM "ob-poc".service_resource_capabilities src
                   JOIN "ob-poc".service_resource_types srt ON srt.resource_id = src.resource_id
                   WHERE src.service_id = $1 AND src.is_active = true
                   ORDER BY src.priority ASC"#
            )
            .bind(service_id)
            .fetch_all(pool)
            .await?;
            
            // Use first (highest priority) resource for each service
            if let Some((resource_id, resource_code, resource_name)) = service_resources.first() {
                resources.push(ResourceToProvision {
                    resource_type_id: *resource_id,
                    resource_code: resource_code.clone(),
                    resource_name: resource_name.clone(),
                    service_id,
                    service_name: service_name.clone(),
                    is_mandatory,
                });
            }
        }
    }
    
    // Deduplicate by resource_type_id
    resources.sort_by_key(|r| r.resource_type_id);
    resources.dedup_by_key(|r| r.resource_type_id);
    
    Ok(resources)
}

/// Build dependency graph from resource_dependencies table
async fn build_resource_dependency_graph(
    pool: &PgPool,
    resources: &[ResourceToProvision],
) -> Result<ResourceDependencyGraph> {
    let resource_ids: Vec<Uuid> = resources.iter().map(|r| r.resource_type_id).collect();
    
    let edges: Vec<(Uuid, Uuid, String, String)> = sqlx::query_as(
        r#"SELECT resource_type_id, depends_on_type_id, dependency_type, inject_arg
           FROM "ob-poc".resource_dependencies
           WHERE resource_type_id = ANY($1) AND is_active = true"#
    )
    .bind(&resource_ids)
    .fetch_all(pool)
    .await?;
    
    let mut graph = ResourceDependencyGraph::new();
    
    for r in resources {
        graph.add_node(r.resource_type_id, r.resource_code.clone());
    }
    
    for (from, to, dep_type, inject_arg) in edges {
        graph.add_edge(from, to, dep_type, inject_arg);
    }
    
    Ok(graph)
}

/// Generate DSL statements with @symbol dependencies
fn generate_provisioning_dsl(
    cbu_id: &Uuid,
    resources: &[ResourceToProvision],
    dep_graph: &ResourceDependencyGraph,
) -> String {
    let mut statements = Vec::new();
    
    for resource in resources {
        let binding = format!("res_{}", resource.resource_code.to_lowercase());
        let url = format!("urn:bnym:{}:{{generated}}", resource.resource_code.to_lowercase());
        
        // Build depends-on list from graph
        let deps: Vec<String> = dep_graph
            .get_dependencies(&resource.resource_type_id)
            .iter()
            .map(|(_, code, _)| format!("@res_{}", code.to_lowercase()))
            .collect();
        
        let depends_on = if deps.is_empty() {
            String::new()
        } else {
            format!(" :depends-on [{}]", deps.join(" "))
        };
        
        statements.push(format!(
            "(service-resource.provision :cbu-id \"{}\" :resource-type \"{}\" :instance-url \"{}\"{} :as @{})",
            cbu_id, resource.resource_code, url, depends_on, binding
        ));
    }
    
    statements.join("\n")
}
```

### 3.3 Add Onboarding Tables

```sql
-- Store generated plans
CREATE TABLE "ob-poc".onboarding_plans (
    plan_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id UUID NOT NULL REFERENCES "ob-poc".cbus(cbu_id),
    products TEXT[] NOT NULL,
    generated_dsl TEXT NOT NULL,
    dependency_graph JSONB NOT NULL,
    resource_count INTEGER NOT NULL,
    status VARCHAR(20) DEFAULT 'pending' 
        CHECK (status IN ('pending', 'modified', 'validated', 'executing', 'complete', 'failed')),
    attribute_overrides JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ DEFAULT now(),
    expires_at TIMESTAMPTZ DEFAULT (now() + interval '24 hours')
);

-- Track execution
CREATE TABLE "ob-poc".onboarding_executions (
    execution_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    plan_id UUID NOT NULL REFERENCES "ob-poc".onboarding_plans(plan_id),
    status VARCHAR(20) DEFAULT 'pending'
        CHECK (status IN ('pending', 'running', 'complete', 'failed', 'cancelled')),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    error_message TEXT,
    result_urls JSONB
);

-- Track per-resource provisioning tasks
CREATE TABLE "ob-poc".onboarding_tasks (
    task_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    execution_id UUID NOT NULL REFERENCES "ob-poc".onboarding_executions(execution_id),
    resource_code VARCHAR(50) NOT NULL,
    resource_instance_id UUID REFERENCES "ob-poc".cbu_resource_instances(instance_id),
    stage INTEGER NOT NULL,  -- Parallel execution stage
    status VARCHAR(20) DEFAULT 'pending'
        CHECK (status IN ('pending', 'running', 'complete', 'failed', 'skipped')),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    error_message TEXT,
    retry_count INTEGER DEFAULT 0
);

CREATE INDEX idx_onboarding_plans_cbu ON "ob-poc".onboarding_plans(cbu_id);
CREATE INDEX idx_onboarding_tasks_exec ON "ob-poc".onboarding_tasks(execution_id);
```

---

## Phase 4: Capability Discovery (Optional Enhancement)

### 4.1 Add `capability` Domain

If you want the full capability-guided model:

```yaml
domains:
  capability:
    description: "Capability discovery and coverage analysis"
    verbs:
      expand:
        description: Expand product to leaf capabilities
        behavior: plugin
        handler: capability_expand
        args:
          - name: product
            type: string
            required: true
        returns:
          type: record_set
          # [{ capability_code, is_leaf, parent_code }]
      
      find-resources:
        description: Find resources that provide a capability
        behavior: plugin
        handler: capability_find_resources
        args:
          - name: capability
            type: string
            required: true
        returns:
          type: record_set
      
      check-coverage:
        description: Check if CBU has capability coverage for a product
        behavior: plugin
        handler: capability_check_coverage
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: product
            type: string
            required: true
        returns:
          type: record
          # { covered: true/false, coverage_percentage: 0.95, gaps: [...] }
      
      gaps:
        description: Identify missing capabilities and suggested resources
        behavior: plugin
        handler: capability_gaps
        args:
          - name: cbu-id
            type: uuid
            required: true
          - name: product
            type: string
            required: true
        returns:
          type: record_set
          # [{ capability, suggested_resources: [...] }]
```

### 4.2 Capability Schema (if needed)

```sql
-- Capability hierarchy
CREATE TABLE "ob-poc".capabilities (
    capability_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    capability_code VARCHAR(50) UNIQUE NOT NULL,
    name VARCHAR(255) NOT NULL,
    parent_code VARCHAR(50) REFERENCES "ob-poc".capabilities(capability_code),
    is_leaf BOOLEAN DEFAULT true,
    description TEXT
);

-- Product requires capabilities
CREATE TABLE "ob-poc".product_capabilities (
    product_id UUID NOT NULL REFERENCES "ob-poc".products(product_id),
    capability_code VARCHAR(50) NOT NULL REFERENCES "ob-poc".capabilities(capability_code),
    is_required BOOLEAN DEFAULT true,
    PRIMARY KEY (product_id, capability_code)
);

-- Resources provide capabilities (alternative to JSONB in service_resource_types)
CREATE TABLE "ob-poc".resource_capabilities (
    resource_type_id UUID NOT NULL REFERENCES "ob-poc".service_resource_types(resource_id),
    capability_code VARCHAR(50) NOT NULL REFERENCES "ob-poc".capabilities(capability_code),
    coverage_strength VARCHAR(20) DEFAULT 'full' CHECK (coverage_strength IN ('full', 'partial', 'optional')),
    PRIMARY KEY (resource_type_id, capability_code)
);
```

---

## Phase 5: Testing & Verification

### 5.1 Integration Test: Full Onboarding Flow

```rust
#[tokio::test]
async fn test_onboarding_full_flow() {
    let pool = test_pool().await;
    
    // Setup: Create CBU
    let cbu_id = create_test_cbu(&pool, "TestFund").await;
    
    // 1. Create plan
    let plan_dsl = r#"(onboarding.plan :cbu-id @fund :products ["CUSTODY_FULL"])"#;
    let mut ctx = ExecutionContext::new();
    ctx.bind("fund", cbu_id);
    
    let result = execute_dsl(&pool, plan_dsl, &mut ctx).await.unwrap();
    let plan_id = result.as_uuid().unwrap();
    
    // 2. Show plan - verify DSL was generated
    let show_dsl = format!(r#"(onboarding.show-plan :plan-id "{}")"#, plan_id);
    let show_result = execute_dsl(&pool, &show_dsl, &mut ctx).await.unwrap();
    let plan_info = show_result.as_record().unwrap();
    
    assert!(plan_info["dsl"].as_str().unwrap().contains("service-resource.provision"));
    assert!(plan_info["dsl"].as_str().unwrap().contains(":depends-on"));
    
    // 3. Execute plan
    let exec_dsl = format!(r#"(onboarding.execute :plan-id "{}")"#, plan_id);
    let exec_result = execute_dsl(&pool, &exec_dsl, &mut ctx).await.unwrap();
    let exec_id = exec_result.as_uuid().unwrap();
    
    // 4. Wait for completion
    let await_dsl = format!(r#"(onboarding.await :execution-id "{}" :timeout 60)"#, exec_id);
    let await_result = execute_dsl(&pool, &await_dsl, &mut ctx).await.unwrap();
    
    assert_eq!(await_result.as_record().unwrap()["status"], "complete");
    
    // 5. Get URLs
    let urls_dsl = format!(r#"(onboarding.get-urls :execution-id "{}")"#, exec_id);
    let urls_result = execute_dsl(&pool, &urls_dsl, &mut ctx).await.unwrap();
    let urls = urls_result.as_record().unwrap();
    
    assert!(urls.contains_key("CASH_ACCOUNT"));
    assert!(urls.contains_key("CUSTODY_ACCOUNT"));
}
```

### 5.2 Unit Test: Dependency Graph Ordering

```rust
#[test]
fn test_dependency_graph_topo_order() {
    let mut graph = ResourceDependencyGraph::new();
    
    graph.add_node(uuid!("1"), "CASH_ACCOUNT");
    graph.add_node(uuid!("2"), "CUSTODY_ACCOUNT");
    graph.add_node(uuid!("3"), "SWIFT_PARSER");
    graph.add_node(uuid!("4"), "SETTLEMENT_INSTR");
    
    // custody depends on cash
    graph.add_edge(uuid!("2"), uuid!("1"), "required", "cash-account-url");
    // swift depends on custody
    graph.add_edge(uuid!("3"), uuid!("2"), "required", "custody-account-url");
    // settlement depends on custody
    graph.add_edge(uuid!("4"), uuid!("2"), "required", "custody-account-url");
    
    let stages = graph.compute_stages().unwrap();
    
    // Stage 0: cash_account (no deps)
    // Stage 1: custody_account (depends on stage 0)
    // Stage 2: swift_parser, settlement_instr (both depend on stage 1, parallel)
    assert_eq!(stages.len(), 3);
    assert!(stages[0].contains(&uuid!("1")));
    assert!(stages[1].contains(&uuid!("2")));
    assert!(stages[2].contains(&uuid!("3")));
    assert!(stages[2].contains(&uuid!("4")));
}
```

---

## Implementation Order

1. **Phase 1.1-1.3** (Schema): Run SQL to create `resource_dependencies` and `resource_instance_dependencies` tables
2. **Phase 3.3** (Schema): Run SQL to create `onboarding_plans`, `onboarding_executions`, `onboarding_tasks` tables
3. **Phase 1.2** (Data): Seed resource dependencies (after verifying service_resource_types has required codes)
4. **Appendix D** (Verify): Run prerequisite checks to ensure taxonomy data exists
5. **Phase 3.1** (Config): Create `rust/config/verbs/onboarding.yaml`
6. **Phase 2.1** (Config): Modify `rust/config/verbs/service-resource.yaml` to add `depends-on` argument
7. **Appendix B** (Code): Create `rust/src/dsl_v2/custom_ops/onboarding.rs` with all handlers
8. **Appendix C** (Code): Register onboarding ops in `rust/src/dsl_v2/custom_ops/mod.rs`
9. **Phase 2.2** (Code): Extend `ResourceCreateOp` in `mod.rs` to handle `depends-on`
10. **Phase 5** (Test): Run integration tests
11. **Phase 4** (Optional): Add capability domain if needed later

Note: Phase 2.3 (topo_sort list handling) is **already verified** - no changes needed.

---

## DSL Usage Examples

### Simple Path
```clojure
(cbu.ensure :name "Acme Hedge Fund" :jurisdiction "US" :as @acme)
(onboarding.ensure :cbu-id @acme :products ["CUSTODY_FULL"])
;; Returns: { status: "complete", urls: { CUSTODY_ACCOUNT: "urn:...", ... } }
```

### Control Path
```clojure
(cbu.ensure :name "Acme Hedge Fund" :jurisdiction "US" :as @acme)
(onboarding.plan :cbu-id @acme :products ["CUSTODY_FULL"] :as @plan)
(onboarding.show-plan :plan-id @plan)
(onboarding.override-attribute :plan-id @plan :resource "CUSTODY_ACCOUNT" :attribute "account_name" :value "ACME-MAIN")
(onboarding.validate-plan :plan-id @plan)
(onboarding.execute :plan-id @plan :as @exec)
(onboarding.await :execution-id @exec :timeout 300)
(onboarding.get-urls :execution-id @exec)
```

### Direct Resource Provisioning (Bypass Workflow)
```clojure
;; Manual provisioning with explicit dependencies
(service-resource.provision :cbu-id @acme :resource-type "CASH_ACCOUNT" 
                            :instance-url "urn:bnym:cash:001" :as @cash)
(service-resource.provision :cbu-id @acme :resource-type "CUSTODY_ACCOUNT"
                            :instance-url "urn:bnym:custody:001" 
                            :depends-on [@cash] :as @custody)
(service-resource.provision :cbu-id @acme :resource-type "SWIFT_PARSER"
                            :instance-url "urn:bnym:swift:001"
                            :depends-on [@custody] :as @swift)
```

---

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| SQL migration (run via psql) | Create | Schema for resource dependencies |
| SQL migration (run via psql) | Create | Schema for onboarding workflow |
| `rust/config/verbs/onboarding.yaml` | Create | Onboarding domain verbs |
| `rust/config/verbs/service-resource.yaml` | Modify | Add `depends-on` argument |
| `rust/src/dsl_v2/custom_ops/onboarding.rs` | Create | Onboarding handlers |
| `rust/src/dsl_v2/custom_ops/mod.rs` | Modify | Register onboarding ops |
| `rust/src/dsl_v2/topo_sort.rs` | Verify | Ensure list args are walked |

---

## Appendix A: Complete Struct Definitions

### A.1 ResourceToProvision

```rust
// In custom_ops/onboarding.rs

/// Resource discovered during product expansion
#[derive(Debug, Clone)]
pub struct ResourceToProvision {
    pub resource_type_id: Uuid,
    pub resource_code: String,
    pub resource_name: String,
    pub service_id: Uuid,
    pub service_name: String,
    pub is_mandatory: bool,
}
```

### A.2 ResourceDependencyGraph

```rust
// In custom_ops/onboarding.rs

use std::collections::{HashMap, HashSet, VecDeque};

/// Graph of resource type dependencies for topological ordering
#[derive(Debug, Clone)]
pub struct ResourceDependencyGraph {
    /// Node data: resource_type_id -> (resource_code, resource_name)
    nodes: HashMap<Uuid, (String, String)>,
    /// Edges: from_resource_id -> [(to_resource_id, inject_arg)]
    edges: HashMap<Uuid, Vec<(Uuid, String)>>,
    /// Reverse edges for in-degree calculation
    reverse_edges: HashMap<Uuid, Vec<Uuid>>,
}

impl ResourceDependencyGraph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
            reverse_edges: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, id: Uuid, code: String) {
        self.nodes.insert(id, (code.clone(), code));
        self.edges.entry(id).or_default();
        self.reverse_edges.entry(id).or_default();
    }

    pub fn add_edge(&mut self, from: Uuid, to: Uuid, _dep_type: String, inject_arg: String) {
        self.edges.entry(from).or_default().push((to, inject_arg));
        self.reverse_edges.entry(to).or_default().push(from);
    }

    /// Get dependencies for a resource: [(dep_id, dep_code, inject_arg)]
    pub fn get_dependencies(&self, resource_id: &Uuid) -> Vec<(Uuid, String, String)> {
        self.edges
            .get(resource_id)
            .map(|deps| {
                deps.iter()
                    .filter_map(|(dep_id, inject_arg)| {
                        self.nodes.get(dep_id).map(|(code, _)| {
                            (*dep_id, code.clone(), inject_arg.clone())
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Compute parallel execution stages using Kahn's algorithm
    pub fn compute_stages(&self) -> Result<Vec<Vec<Uuid>>, String> {
        let mut in_degree: HashMap<Uuid, usize> = HashMap::new();
        
        // Initialize in-degrees
        for id in self.nodes.keys() {
            in_degree.insert(*id, self.reverse_edges.get(id).map(|v| v.len()).unwrap_or(0));
        }

        let mut stages: Vec<Vec<Uuid>> = Vec::new();
        let mut remaining: HashSet<Uuid> = self.nodes.keys().copied().collect();

        while !remaining.is_empty() {
            // Find all nodes with in_degree == 0
            let stage: Vec<Uuid> = remaining
                .iter()
                .filter(|id| in_degree.get(id).copied().unwrap_or(0) == 0)
                .copied()
                .collect();

            if stage.is_empty() {
                return Err("Cycle detected in resource dependencies".to_string());
            }

            // Remove this stage from remaining, decrement dependents
            for id in &stage {
                remaining.remove(id);
                if let Some(dependents) = self.edges.get(id) {
                    for (dep_id, _) in dependents {
                        if let Some(deg) = in_degree.get_mut(dep_id) {
                            *deg = deg.saturating_sub(1);
                        }
                    }
                }
            }

            stages.push(stage);
        }

        Ok(stages)
    }

    /// Serialize to JSON for storage
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "nodes": self.nodes.iter().map(|(id, (code, name))| {
                serde_json::json!({"id": id, "code": code, "name": name})
            }).collect::<Vec<_>>(),
            "edges": self.edges.iter().flat_map(|(from, tos)| {
                tos.iter().map(move |(to, inject)| {
                    serde_json::json!({"from": from, "to": to, "inject_arg": inject})
                })
            }).collect::<Vec<_>>()
        })
    }
}
```

---

## Appendix B: Complete Handler Implementations

### B.1 Helper Functions

```rust
// In custom_ops/onboarding.rs

use anyhow::{anyhow, Result};
use uuid::Uuid;

/// Extract UUID from verb argument (handles @symbol and literal)
fn extract_uuid(verb_call: &VerbCall, ctx: &ExecutionContext, arg_name: &str) -> Result<Uuid> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == arg_name)
        .and_then(|a| {
            if let Some(name) = a.value.as_symbol() {
                ctx.resolve(name)
            } else {
                a.value.as_uuid()
            }
        })
        .ok_or_else(|| anyhow!("Missing {} argument", arg_name))
}

/// Extract string list from verb argument
fn extract_string_list(verb_call: &VerbCall, arg_name: &str) -> Result<Vec<String>> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == arg_name)
        .and_then(|a| a.value.as_list())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_string().map(|s| s.to_string()))
                .collect()
        })
        .ok_or_else(|| anyhow!("Missing {} argument", arg_name))
}

/// Store onboarding plan in database
async fn store_onboarding_plan(
    pool: &PgPool,
    cbu_id: Uuid,
    products: &[String],
    dsl: &str,
    dep_graph: &ResourceDependencyGraph,
) -> Result<Uuid> {
    let plan_id = Uuid::new_v4();
    let resource_count = dep_graph.nodes.len() as i32;
    
    sqlx::query(
        r#"INSERT INTO "ob-poc".onboarding_plans 
           (plan_id, cbu_id, products, generated_dsl, dependency_graph, resource_count)
           VALUES ($1, $2, $3, $4, $5, $6)"#
    )
    .bind(plan_id)
    .bind(cbu_id)
    .bind(products)
    .bind(dsl)
    .bind(dep_graph.to_json())
    .bind(resource_count)
    .execute(pool)
    .await?;
    
    Ok(plan_id)
}
```

### B.2 OnboardingPlanOp (Complete)

```rust
pub struct OnboardingPlanOp;

#[async_trait]
impl CustomOperation for OnboardingPlanOp {
    fn domain(&self) -> &'static str { "onboarding" }
    fn verb(&self) -> &'static str { "plan" }
    fn rationale(&self) -> &'static str {
        "Requires product→service→resource expansion and dependency graph traversal"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let products = extract_string_list(verb_call, "products")?;
        
        // 1. Expand products to resource types
        let resources = expand_products_to_resources(pool, &products).await?;
        
        if resources.is_empty() {
            return Err(anyhow!("No resources found for products: {:?}", products));
        }
        
        // 2. Build dependency graph
        let dep_graph = build_resource_dependency_graph(pool, &resources).await?;
        
        // 3. Generate DSL statements
        let dsl = generate_provisioning_dsl(&cbu_id, &resources, &dep_graph);
        
        // 4. Store plan
        let plan_id = store_onboarding_plan(pool, cbu_id, &products, &dsl, &dep_graph).await?;
        
        ctx.bind("plan", plan_id);
        Ok(ExecutionResult::Uuid(plan_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}
```

### B.3 OnboardingShowPlanOp

```rust
pub struct OnboardingShowPlanOp;

#[async_trait]
impl CustomOperation for OnboardingShowPlanOp {
    fn domain(&self) -> &'static str { "onboarding" }
    fn verb(&self) -> &'static str { "show-plan" }
    fn rationale(&self) -> &'static str {
        "Retrieves and formats stored plan for display"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let plan_id = extract_uuid(verb_call, ctx, "plan-id")?;
        
        let row: Option<(String, serde_json::Value, i32, String)> = sqlx::query_as(
            r#"SELECT generated_dsl, dependency_graph, resource_count, status
               FROM "ob-poc".onboarding_plans WHERE plan_id = $1"#
        )
        .bind(plan_id)
        .fetch_optional(pool)
        .await?;
        
        match row {
            Some((dsl, graph, count, status)) => {
                Ok(ExecutionResult::Record(serde_json::json!({
                    "plan_id": plan_id,
                    "dsl": dsl,
                    "dependency_graph": graph,
                    "resource_count": count,
                    "status": status
                })))
            }
            None => Err(anyhow!("Plan not found: {}", plan_id))
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({})))
    }
}
```

### B.4 OnboardingExecuteOp

```rust
pub struct OnboardingExecuteOp;

#[async_trait]
impl CustomOperation for OnboardingExecuteOp {
    fn domain(&self) -> &'static str { "onboarding" }
    fn verb(&self) -> &'static str { "execute" }
    fn rationale(&self) -> &'static str {
        "Compiles and executes generated DSL with stage-based parallelism"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::dsl_v2::parser::parse_program;
        use crate::dsl_v2::topo_sort::topological_sort;
        use crate::dsl_v2::binding_context::BindingContext;
        
        let plan_id = extract_uuid(verb_call, ctx, "plan-id")?;
        let parallel = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "parallel")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(true);
        
        // Load plan
        let row: Option<(Uuid, String, serde_json::Value)> = sqlx::query_as(
            r#"SELECT cbu_id, generated_dsl, dependency_graph
               FROM "ob-poc".onboarding_plans WHERE plan_id = $1"#
        )
        .bind(plan_id)
        .fetch_optional(pool)
        .await?;
        
        let (cbu_id, dsl, _graph) = row.ok_or_else(|| anyhow!("Plan not found: {}", plan_id))?;
        
        // Create execution record
        let execution_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO "ob-poc".onboarding_executions 
               (execution_id, plan_id, status, started_at)
               VALUES ($1, $2, 'running', NOW())"#
        )
        .bind(execution_id)
        .bind(plan_id)
        .execute(pool)
        .await?;
        
        // Update plan status
        sqlx::query(r#"UPDATE "ob-poc".onboarding_plans SET status = 'executing' WHERE plan_id = $1"#)
            .bind(plan_id)
            .execute(pool)
            .await?;
        
        // Parse and execute the generated DSL
        // The DSL contains @symbol dependencies that topological_sort will order correctly
        let program = parse_program(&dsl).map_err(|e| anyhow!("Parse error: {:?}", e))?;
        
        // Bind cbu_id so @cbu references resolve
        let mut binding_ctx = BindingContext::new();
        // Note: The generated DSL uses literal UUIDs, not @symbols for cbu-id
        
        // Execute via standard executor (which handles topo sort internally)
        // For now, execute synchronously - async/parallel execution is a future enhancement
        let result = crate::dsl_v2::executor::execute_program(pool, &program, ctx).await;
        
        // Update execution status
        match &result {
            Ok(_) => {
                sqlx::query(
                    r#"UPDATE "ob-poc".onboarding_executions 
                       SET status = 'complete', completed_at = NOW()
                       WHERE execution_id = $1"#
                )
                .bind(execution_id)
                .execute(pool)
                .await?;
                
                sqlx::query(r#"UPDATE "ob-poc".onboarding_plans SET status = 'complete' WHERE plan_id = $1"#)
                    .bind(plan_id)
                    .execute(pool)
                    .await?;
            }
            Err(e) => {
                sqlx::query(
                    r#"UPDATE "ob-poc".onboarding_executions 
                       SET status = 'failed', completed_at = NOW(), error_message = $2
                       WHERE execution_id = $1"#
                )
                .bind(execution_id)
                .bind(e.to_string())
                .execute(pool)
                .await?;
                
                sqlx::query(r#"UPDATE "ob-poc".onboarding_plans SET status = 'failed' WHERE plan_id = $1"#)
                    .bind(plan_id)
                    .execute(pool)
                    .await?;
            }
        }
        
        result?;
        
        ctx.bind("execution", execution_id);
        Ok(ExecutionResult::Uuid(execution_id))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Uuid(Uuid::new_v4()))
    }
}
```

### B.5 OnboardingEnsureOp

```rust
pub struct OnboardingEnsureOp;

#[async_trait]
impl CustomOperation for OnboardingEnsureOp {
    fn domain(&self) -> &'static str { "onboarding" }
    fn verb(&self) -> &'static str { "ensure" }
    fn rationale(&self) -> &'static str {
        "Idempotent plan+execute in single operation"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;
        let products = extract_string_list(verb_call, "products")?;
        
        // Check for existing complete onboarding
        let existing: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT plan_id FROM "ob-poc".onboarding_plans 
               WHERE cbu_id = $1 AND products = $2 AND status = 'complete'
               LIMIT 1"#
        )
        .bind(cbu_id)
        .bind(&products)
        .fetch_optional(pool)
        .await?;
        
        if let Some(plan_id) = existing {
            // Already onboarded - return existing resource URLs
            let urls: Option<serde_json::Value> = sqlx::query_scalar(
                r#"SELECT result_urls FROM "ob-poc".onboarding_executions 
                   WHERE plan_id = $1 AND status = 'complete'
                   ORDER BY completed_at DESC LIMIT 1"#
            )
            .bind(plan_id)
            .fetch_optional(pool)
            .await?;
            
            return Ok(ExecutionResult::Record(serde_json::json!({
                "status": "already_complete",
                "plan_id": plan_id,
                "urls": urls.unwrap_or(serde_json::json!({}))
            })));
        }
        
        // Create plan
        let resources = expand_products_to_resources(pool, &products).await?;
        let dep_graph = build_resource_dependency_graph(pool, &resources).await?;
        let dsl = generate_provisioning_dsl(&cbu_id, &resources, &dep_graph);
        let plan_id = store_onboarding_plan(pool, cbu_id, &products, &dsl, &dep_graph).await?;
        
        // Execute (reuse execute logic)
        let exec_verb = VerbCall {
            domain: "onboarding".to_string(),
            verb: "execute".to_string(),
            arguments: vec![
                super::ast::Argument {
                    key: "plan-id".to_string(),
                    value: super::ast::AstNode::Literal(super::ast::Literal::Uuid(plan_id)),
                    span: super::ast::Span::synthetic(),
                }
            ],
            binding: None,
            span: super::ast::Span::synthetic(),
        };
        
        let exec_op = OnboardingExecuteOp;
        let exec_result = exec_op.execute(&exec_verb, ctx, pool).await?;
        
        // Collect URLs
        let urls = collect_provisioned_urls(pool, plan_id).await?;
        
        Ok(ExecutionResult::Record(serde_json::json!({
            "status": "complete",
            "plan_id": plan_id,
            "urls": urls
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(serde_json::json!({"status": "complete"})))
    }
}

/// Collect provisioned resource URLs for a plan
async fn collect_provisioned_urls(pool: &PgPool, plan_id: Uuid) -> Result<serde_json::Value> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT srt.resource_code, cri.instance_url
           FROM "ob-poc".onboarding_tasks ot
           JOIN "ob-poc".onboarding_executions oe ON oe.execution_id = ot.execution_id
           JOIN "ob-poc".cbu_resource_instances cri ON cri.instance_id = ot.resource_instance_id
           JOIN "ob-poc".service_resource_types srt ON srt.resource_id = cri.resource_type_id
           WHERE oe.plan_id = $1 AND ot.status = 'complete'"#
    )
    .bind(plan_id)
    .fetch_all(pool)
    .await?;
    
    let mut urls = serde_json::Map::new();
    for (code, url) in rows {
        urls.insert(code, serde_json::Value::String(url));
    }
    
    Ok(serde_json::Value::Object(urls))
}
```

---

## Appendix C: Registration in mod.rs

Add to `rust/src/dsl_v2/custom_ops/mod.rs`:

```rust
// At top of file, add module declaration
mod onboarding;

// In the pub use section
pub use onboarding::{
    OnboardingPlanOp, OnboardingShowPlanOp, OnboardingExecuteOp, 
    OnboardingEnsureOp, OnboardingGetUrlsOp,
};

// In CustomOperationRegistry::new(), add registrations
impl CustomOperationRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            operations: HashMap::new(),
        };

        // ... existing registrations ...

        // Onboarding operations
        registry.register(Arc::new(OnboardingPlanOp));
        registry.register(Arc::new(OnboardingShowPlanOp));
        registry.register(Arc::new(OnboardingExecuteOp));
        registry.register(Arc::new(OnboardingEnsureOp));
        registry.register(Arc::new(OnboardingGetUrlsOp));

        registry
    }
}
```

---

## Appendix D: Prerequisites Check

Before implementation, verify these exist in the database:

```sql
-- Check service_resource_types has required codes
SELECT resource_code, name FROM "ob-poc".service_resource_types 
WHERE resource_code IN ('CASH_ACCOUNT', 'CUSTODY_ACCOUNT', 'SWIFT_PARSER', 'SETTLEMENT_INSTRUCTIONS');

-- Check products exist
SELECT product_code, name FROM "ob-poc".products WHERE product_code = 'CUSTODY_FULL';

-- Check product_services links exist
SELECT p.product_code, s.name 
FROM "ob-poc".product_services ps
JOIN "ob-poc".products p ON p.product_id = ps.product_id
JOIN "ob-poc".services s ON s.service_id = ps.service_id;

-- Check service_resource_capabilities links exist
SELECT s.name as service, srt.resource_code as resource
FROM "ob-poc".service_resource_capabilities src
JOIN "ob-poc".services s ON s.service_id = src.service_id
JOIN "ob-poc".service_resource_types srt ON srt.resource_id = src.resource_id;
```

If these are empty, seed data is required before the onboarding workflow will work.

---

## Appendix E: URL Generation Strategy

The `generate_provisioning_dsl` function currently uses placeholder URLs:

```rust
let url = format!("urn:bnym:{}:{{generated}}", resource.resource_code.to_lowercase());
```

**Option 1: Pre-generate URLs**
```rust
let url = format!("urn:bnym:{}:{}", 
    resource.resource_code.to_lowercase(),
    Uuid::new_v4()
);
```

**Option 2: Let ResourceCreateOp generate**
Pass `instance-url` as empty/placeholder, have `ResourceCreateOp` generate if missing:
```rust
// In ResourceCreateOp::execute()
let instance_url = verb_call
    .arguments
    .iter()
    .find(|a| a.key == "instance-url")
    .and_then(|a| a.value.as_string())
    .unwrap_or_else(|| format!("urn:bnym:{}:{}", resource_type_code, Uuid::new_v4()));
```

**Recommendation**: Option 2 - let the handler generate URLs, making the DSL simpler.