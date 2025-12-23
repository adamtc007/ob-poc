//! Onboarding Workflow Operations
//!
//! These operations orchestrate CBU onboarding to products by:
//! 1. Expanding products → services → resources
//! 2. Building dependency graph from entity_type_dependencies table (unified DAG)
//! 3. Generating DSL statements with @symbol dependencies
//! 4. Delegating to standard DSL compiler for ordering
//! 5. Executing stages (optionally in parallel)
//!
//! Rationale: Requires graph traversal, DSL generation, and multi-step orchestration
//! that cannot be expressed as simple CRUD operations.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::{CustomOperation, ExecutionContext, ExecutionResult, VerbCall};
use crate::dsl_v2::entity_deps::EntityDependencyRegistry;
#[allow(unused_imports)]
use crate::dsl_v2::entity_deps::{topological_sort_unified, EntityInstance};

#[cfg(feature = "database")]
use sqlx::PgPool;

// =============================================================================
// TYPES
// =============================================================================

/// Resource discovered during product expansion
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used for display/logging in future iterations
pub struct ResourceToProvision {
    pub resource_type_id: Uuid,
    pub resource_code: String,
    pub resource_name: String,
    pub service_id: Uuid,
    pub service_name: String,
    pub is_mandatory: bool,
}

/// Graph of resource type dependencies for topological ordering
///
/// **DEPRECATED**: This struct is being replaced by the unified `EntityDependencyRegistry`
/// from `crate::dsl_v2::entity_deps`. The new system:
/// - Uses string-based type/subtype instead of UUIDs
/// - Supports all entity types (CBU, case, fund, resource, etc.)
/// - Provides `topological_sort_unified()` for generic ordering
///
/// Migration path: Use `EntityDependencyRegistry::load()` and `topological_sort_unified()`
/// instead of `ResourceDependencyGraph::compute_stages()`.
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
                        self.nodes
                            .get(dep_id)
                            .map(|(code, _)| (*dep_id, code.clone(), inject_arg.clone()))
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Compute parallel execution stages using Kahn's algorithm
    pub fn compute_stages(&self) -> Result<Vec<Vec<Uuid>>, String> {
        let mut in_degree: HashMap<Uuid, usize> = HashMap::new();

        // Initialize in-degrees (count how many dependencies each node has)
        // A node with 0 dependencies (out-edges) is ready to execute first
        for id in self.nodes.keys() {
            in_degree.insert(*id, self.edges.get(id).map(|v| v.len()).unwrap_or(0));
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

            // Remove this stage from remaining, decrement nodes that depend on completed ones
            for id in &stage {
                remaining.remove(id);
                // Find nodes that depend on this one (via reverse_edges)
                if let Some(dependents) = self.reverse_edges.get(id) {
                    for dep_id in dependents {
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

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Extract UUID from verb argument (handles @symbol and literal)
fn extract_uuid(verb_call: &VerbCall, ctx: &ExecutionContext, arg_name: &str) -> Result<Uuid> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == arg_name)
        .and_then(|a| {
            // Try symbol reference first
            if let Some(sym) = a.value.as_symbol() {
                return ctx.resolve(sym);
            }
            // Try literal UUID
            a.value.as_uuid()
        })
        .ok_or_else(|| anyhow!("Missing {} argument", arg_name))
}

/// Extract optional UUID from verb argument (handles @symbol and literal)
fn extract_uuid_opt(verb_call: &VerbCall, ctx: &ExecutionContext, arg_name: &str) -> Option<Uuid> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == arg_name)
        .and_then(|a| {
            // Try symbol reference first
            if let Some(sym) = a.value.as_symbol() {
                return ctx.resolve(sym);
            }
            // Try literal UUID
            a.value.as_uuid()
        })
}

/// Extract optional string from verb argument
fn extract_string_opt(verb_call: &VerbCall, arg_name: &str) -> Option<String> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == arg_name)
        .and_then(|a| a.value.as_string().map(|s| s.to_string()))
}

/// Resolve CBU by either cbu-id or cbu-name
#[cfg(feature = "database")]
async fn resolve_cbu_id(
    verb_call: &VerbCall,
    ctx: &ExecutionContext,
    pool: &PgPool,
) -> Result<Uuid> {
    // First try cbu-id (direct UUID or @symbol reference)
    if let Some(cbu_id) = extract_uuid_opt(verb_call, ctx, "cbu-id") {
        return Ok(cbu_id);
    }

    // Fall back to cbu-name lookup
    if let Some(cbu_name) = extract_string_opt(verb_call, "cbu-name") {
        let row: Option<(Uuid,)> =
            sqlx::query_as(r#"SELECT cbu_id FROM "ob-poc".cbus WHERE name = $1"#)
                .bind(&cbu_name)
                .fetch_optional(pool)
                .await?;

        return row
            .map(|(id,)| id)
            .ok_or_else(|| anyhow!("CBU not found: {}", cbu_name));
    }

    Err(anyhow!("Missing cbu-id or cbu-name argument"))
}

/// Extract string list from verb argument
fn extract_string_list(verb_call: &VerbCall, arg_name: &str) -> Result<Vec<String>> {
    verb_call
        .arguments
        .iter()
        .find(|a| a.key == arg_name)
        .and_then(|a| {
            a.value.as_list().map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_string().map(|s| s.to_string()))
                    .collect()
            })
        })
        .ok_or_else(|| anyhow!("Missing {} argument", arg_name))
}

/// Normalize products array for consistent comparison (sort alphabetically)
fn normalize_products(products: &[String]) -> Vec<String> {
    let mut sorted = products.to_vec();
    sorted.sort();
    sorted
}

#[cfg(feature = "database")]
async fn expand_products_to_resources(
    pool: &PgPool,
    products: &[String],
) -> Result<Vec<ResourceToProvision>> {
    let mut resources = Vec::new();

    for product_code in products {
        // Get services for this product
        let services: Vec<(Uuid, String, bool)> = sqlx::query_as(
            r#"SELECT s.service_id, s.name, COALESCE(ps.is_mandatory, false)
               FROM "ob-poc".product_services ps
               JOIN "ob-poc".services s ON s.service_id = ps.service_id
               JOIN "ob-poc".products p ON p.product_id = ps.product_id
               WHERE p.product_code = $1 AND s.is_active = true"#,
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
                   ORDER BY src.priority ASC"#,
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

/// Build resource dependency graph using the unified entity_type_dependencies table
#[cfg(feature = "database")]
async fn build_resource_dependency_graph_unified(
    pool: &PgPool,
    resources: &[ResourceToProvision],
) -> Result<(ResourceDependencyGraph, EntityDependencyRegistry)> {
    // Load the unified dependency registry
    let registry = EntityDependencyRegistry::load(pool).await?;

    // Build code -> resource mapping for lookup
    let code_to_resource: HashMap<&str, &ResourceToProvision> = resources
        .iter()
        .map(|r| (r.resource_code.as_str(), r))
        .collect();

    let mut graph = ResourceDependencyGraph::new();

    // Add all resources as nodes
    for r in resources {
        graph.add_node(r.resource_type_id, r.resource_code.clone());
    }

    // Add edges from unified registry
    // Note: The unified table uses "resource_instance" as the type
    for r in resources {
        let deps = registry.dependencies_of("resource_instance", Some(&r.resource_code));
        for dep in deps {
            // Check if the dependency target is in our resource set
            if let Some(ref to_subtype) = dep.to_subtype {
                if let Some(target) = code_to_resource.get(to_subtype.as_str()) {
                    let inject_arg = dep.via_arg.clone().unwrap_or_default();
                    graph.add_edge(
                        r.resource_type_id,
                        target.resource_type_id,
                        dep.kind.to_string(),
                        inject_arg,
                    );
                }
            }
        }
    }

    Ok((graph, registry))
}

/// Build EntityInstance list for unified topological sort
/// This will be used when we fully migrate to the unified DAG
#[allow(dead_code)]
fn build_entity_instances(
    resources: &[ResourceToProvision],
    registry: &EntityDependencyRegistry,
) -> Vec<EntityInstance> {
    // Build code -> binding name mapping
    let code_to_binding: HashMap<&str, String> = resources
        .iter()
        .map(|r| {
            (
                r.resource_code.as_str(),
                format!("res_{}", r.resource_code.to_lowercase()),
            )
        })
        .collect();

    resources
        .iter()
        .map(|r| {
            let binding = format!("res_{}", r.resource_code.to_lowercase());

            // Get dependencies from registry
            // Note: The unified table uses "resource_instance" as the type
            let deps = registry.dependencies_of("resource_instance", Some(&r.resource_code));
            let depends_on: Vec<String> = deps
                .iter()
                .filter_map(|dep| {
                    dep.to_subtype
                        .as_ref()
                        .and_then(|to_subtype| code_to_binding.get(to_subtype.as_str()).cloned())
                })
                .collect();

            EntityInstance {
                id: binding,
                entity_type: "resource".to_string(),
                subtype: Some(r.resource_code.clone()),
                depends_on,
            }
        })
        .collect()
}

/// Legacy: Build resource dependency graph from old resource_dependencies table
/// This is deprecated and will be removed once migration to entity_type_dependencies is complete
#[cfg(feature = "database")]
#[allow(dead_code)]
async fn build_resource_dependency_graph_legacy(
    pool: &PgPool,
    resources: &[ResourceToProvision],
) -> Result<ResourceDependencyGraph> {
    let resource_ids: Vec<Uuid> = resources.iter().map(|r| r.resource_type_id).collect();

    let edges: Vec<(Uuid, Uuid, String, String)> = sqlx::query_as(
        r#"SELECT resource_type_id, depends_on_type_id, dependency_type, inject_arg
           FROM "ob-poc".resource_dependencies
           WHERE resource_type_id = ANY($1) AND is_active = true"#,
    )
    .bind(&resource_ids)
    .fetch_all(pool)
    .await?;

    let mut graph = ResourceDependencyGraph::new();

    for r in resources {
        graph.add_node(r.resource_type_id, r.resource_code.clone());
    }

    for (from, to, dep_type, inject_arg) in edges {
        // Only add edge if the dependency is also in our resource set
        if resource_ids.contains(&to) {
            graph.add_edge(from, to, dep_type, inject_arg);
        }
    }

    Ok(graph)
}

/// Build resource dependency graph - uses unified entity_type_dependencies
#[cfg(feature = "database")]
async fn build_resource_dependency_graph(
    pool: &PgPool,
    resources: &[ResourceToProvision],
) -> Result<ResourceDependencyGraph> {
    let (graph, _registry) = build_resource_dependency_graph_unified(pool, resources).await?;
    Ok(graph)
}

/// Generate DSL statements with @symbol dependencies
fn generate_provisioning_dsl(
    cbu_id: &Uuid,
    resources: &[ResourceToProvision],
    dep_graph: &ResourceDependencyGraph,
) -> String {
    let mut statements = Vec::new();

    // Compute stages to ensure proper ordering in DSL
    let stages = dep_graph.compute_stages().unwrap_or_else(|_| {
        // Fallback: all resources in single stage
        vec![resources.iter().map(|r| r.resource_type_id).collect()]
    });

    // Build resource_id -> code mapping
    let id_to_code: HashMap<Uuid, String> = resources
        .iter()
        .map(|r| (r.resource_type_id, r.resource_code.clone()))
        .collect();

    // Generate statements in stage order
    for stage in stages {
        for resource_id in stage {
            if let Some(resource) = resources.iter().find(|r| r.resource_type_id == resource_id) {
                let binding = format!("res_{}", resource.resource_code.to_lowercase());

                // Build depends-on list from graph
                let deps: Vec<String> = dep_graph
                    .get_dependencies(&resource.resource_type_id)
                    .iter()
                    .filter_map(|(dep_id, _, _)| {
                        id_to_code
                            .get(dep_id)
                            .map(|code| format!("@res_{}", code.to_lowercase()))
                    })
                    .collect();

                let depends_on = if deps.is_empty() {
                    String::new()
                } else {
                    format!(" :depends-on [{}]", deps.join(" "))
                };

                statements.push(format!(
                    "(service-resource.provision :cbu-id \"{}\" :resource-type \"{}\"{} :as @{})",
                    cbu_id, resource.resource_code, depends_on, binding
                ));
            }
        }
    }

    statements.join("\n")
}

#[cfg(feature = "database")]
async fn store_onboarding_plan(
    pool: &PgPool,
    cbu_id: Uuid,
    products: &[String],
    dsl: &str,
    dep_graph: &ResourceDependencyGraph,
) -> Result<Uuid> {
    let plan_id = Uuid::new_v4();
    let resource_count = dep_graph.nodes.len() as i32;
    // Normalize products for consistent storage/comparison
    let normalized_products = normalize_products(products);

    sqlx::query(
        r#"INSERT INTO "ob-poc".onboarding_plans
           (plan_id, cbu_id, products, generated_dsl, dependency_graph, resource_count)
           VALUES ($1, $2, $3, $4, $5, $6)"#,
    )
    .bind(plan_id)
    .bind(cbu_id)
    .bind(&normalized_products)
    .bind(dsl)
    .bind(dep_graph.to_json())
    .bind(resource_count)
    .execute(pool)
    .await?;

    Ok(plan_id)
}

#[cfg(feature = "database")]
async fn collect_provisioned_urls(pool: &PgPool, execution_id: Uuid) -> Result<serde_json::Value> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT srt.resource_code, cri.instance_url
           FROM "ob-poc".onboarding_tasks ot
           JOIN "ob-poc".cbu_resource_instances cri ON cri.instance_id = ot.resource_instance_id
           JOIN "ob-poc".service_resource_types srt ON srt.resource_id = cri.resource_type_id
           WHERE ot.execution_id = $1 AND ot.status = 'complete'"#,
    )
    .bind(execution_id)
    .fetch_all(pool)
    .await?;

    let mut urls = serde_json::Map::new();
    for (code, url) in rows {
        urls.insert(code, serde_json::Value::String(url));
    }

    Ok(serde_json::Value::Object(urls))
}

// =============================================================================
// OPERATIONS
// =============================================================================

/// onboarding.plan - Create onboarding plan by expanding products to resources
pub struct OnboardingPlanOp;

#[async_trait]
impl CustomOperation for OnboardingPlanOp {
    fn domain(&self) -> &'static str {
        "onboarding"
    }
    fn verb(&self) -> &'static str {
        "plan"
    }
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
        let cbu_id = resolve_cbu_id(verb_call, ctx, pool).await?;
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

/// onboarding.show-plan - Display generated DSL and dependency graph
pub struct OnboardingShowPlanOp;

#[async_trait]
impl CustomOperation for OnboardingShowPlanOp {
    fn domain(&self) -> &'static str {
        "onboarding"
    }
    fn verb(&self) -> &'static str {
        "show-plan"
    }
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
               FROM "ob-poc".onboarding_plans WHERE plan_id = $1"#,
        )
        .bind(plan_id)
        .fetch_optional(pool)
        .await?;

        match row {
            Some((dsl, graph, count, status)) => Ok(ExecutionResult::Record(serde_json::json!({
                "plan_id": plan_id,
                "dsl": dsl,
                "dependency_graph": graph,
                "resource_count": count,
                "status": status
            }))),
            None => Err(anyhow!("Plan not found: {}", plan_id)),
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

/// onboarding.execute - Compile and execute plan's DSL statements
pub struct OnboardingExecuteOp;

#[async_trait]
impl CustomOperation for OnboardingExecuteOp {
    fn domain(&self) -> &'static str {
        "onboarding"
    }
    fn verb(&self) -> &'static str {
        "execute"
    }
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
        let plan_id = extract_uuid(verb_call, ctx, "plan-id")?;

        // Load plan
        let row: Option<(Uuid, String, serde_json::Value)> = sqlx::query_as(
            r#"SELECT cbu_id, generated_dsl, dependency_graph
               FROM "ob-poc".onboarding_plans WHERE plan_id = $1"#,
        )
        .bind(plan_id)
        .fetch_optional(pool)
        .await?;

        let (_cbu_id, dsl, _graph) = row.ok_or_else(|| anyhow!("Plan not found: {}", plan_id))?;

        // Create execution record
        let execution_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO "ob-poc".onboarding_executions
               (execution_id, plan_id, status, started_at)
               VALUES ($1, $2, 'running', NOW())"#,
        )
        .bind(execution_id)
        .bind(plan_id)
        .execute(pool)
        .await?;

        // Update plan status
        sqlx::query(
            r#"UPDATE "ob-poc".onboarding_plans SET status = 'executing' WHERE plan_id = $1"#,
        )
        .bind(plan_id)
        .execute(pool)
        .await?;

        // Execute the generated DSL via the standard executor
        let executor = crate::dsl_v2::executor::DslExecutor::new(pool.clone());
        let result = executor.execute_dsl(&dsl, ctx).await;

        // Update execution status
        match &result {
            Ok(_) => {
                sqlx::query(
                    r#"UPDATE "ob-poc".onboarding_executions
                       SET status = 'complete', completed_at = NOW()
                       WHERE execution_id = $1"#,
                )
                .bind(execution_id)
                .execute(pool)
                .await?;

                sqlx::query(
                    r#"UPDATE "ob-poc".onboarding_plans SET status = 'complete' WHERE plan_id = $1"#,
                )
                .bind(plan_id)
                .execute(pool)
                .await?;
            }
            Err(e) => {
                sqlx::query(
                    r#"UPDATE "ob-poc".onboarding_executions
                       SET status = 'failed', completed_at = NOW(), error_message = $2
                       WHERE execution_id = $1"#,
                )
                .bind(execution_id)
                .bind(e.to_string())
                .execute(pool)
                .await?;

                sqlx::query(
                    r#"UPDATE "ob-poc".onboarding_plans SET status = 'failed' WHERE plan_id = $1"#,
                )
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

/// onboarding.status - Check execution progress
pub struct OnboardingStatusOp;

#[async_trait]
impl CustomOperation for OnboardingStatusOp {
    fn domain(&self) -> &'static str {
        "onboarding"
    }
    fn verb(&self) -> &'static str {
        "status"
    }
    fn rationale(&self) -> &'static str {
        "Aggregates task status for execution progress reporting"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let execution_id = extract_uuid(verb_call, ctx, "execution-id")?;

        let row: Option<(String, Option<String>)> = sqlx::query_as(
            r#"SELECT status, error_message
               FROM "ob-poc".onboarding_executions WHERE execution_id = $1"#,
        )
        .bind(execution_id)
        .fetch_optional(pool)
        .await?;

        match row {
            Some((status, error_message)) => {
                // Get task counts
                let task_counts: (i64, i64, i64, i64) = sqlx::query_as(
                    r#"SELECT
                           COUNT(*) FILTER (WHERE status = 'complete') as completed,
                           COUNT(*) FILTER (WHERE status = 'running') as running,
                           COUNT(*) FILTER (WHERE status = 'pending') as pending,
                           COUNT(*) FILTER (WHERE status = 'failed') as failed
                       FROM "ob-poc".onboarding_tasks WHERE execution_id = $1"#,
                )
                .bind(execution_id)
                .fetch_one(pool)
                .await
                .unwrap_or((0, 0, 0, 0));

                Ok(ExecutionResult::Record(serde_json::json!({
                    "execution_id": execution_id,
                    "status": status,
                    "error_message": error_message,
                    "completed": task_counts.0,
                    "running": task_counts.1,
                    "pending": task_counts.2,
                    "failed": task_counts.3,
                    "total": task_counts.0 + task_counts.1 + task_counts.2 + task_counts.3
                })))
            }
            None => Err(anyhow!("Execution not found: {}", execution_id)),
        }
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(
            serde_json::json!({"status": "complete"}),
        ))
    }
}

/// onboarding.get-urls - Retrieve resource URLs after execution
pub struct OnboardingGetUrlsOp;

#[async_trait]
impl CustomOperation for OnboardingGetUrlsOp {
    fn domain(&self) -> &'static str {
        "onboarding"
    }
    fn verb(&self) -> &'static str {
        "get-urls"
    }
    fn rationale(&self) -> &'static str {
        "Aggregates provisioned resource URLs from execution tasks"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        // Support multiple lookup methods:
        // 1. By execution-id directly
        // 2. By cbu-id or cbu-name (finds most recent completed execution)

        if let Some(execution_id) = extract_uuid_opt(verb_call, ctx, "execution-id") {
            let urls = collect_provisioned_urls(pool, execution_id).await?;
            return Ok(ExecutionResult::Record(urls));
        }

        // Try to resolve by CBU
        let cbu_id = resolve_cbu_id(verb_call, ctx, pool).await?;

        // Find most recent completed execution for this CBU
        let execution_id: Option<Uuid> = sqlx::query_scalar(
            r#"SELECT oe.execution_id
               FROM "ob-poc".onboarding_executions oe
               JOIN "ob-poc".onboarding_plans op ON op.plan_id = oe.plan_id
               WHERE op.cbu_id = $1 AND oe.status = 'complete'
               ORDER BY oe.completed_at DESC
               LIMIT 1"#,
        )
        .bind(cbu_id)
        .fetch_optional(pool)
        .await?;

        match execution_id {
            Some(exec_id) => {
                let urls = collect_provisioned_urls(pool, exec_id).await?;
                Ok(ExecutionResult::Record(urls))
            }
            None => Ok(ExecutionResult::Record(serde_json::json!({
                "error": "No completed onboarding execution found for this CBU",
                "cbu_id": cbu_id
            }))),
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

/// onboarding.ensure - Idempotent plan + execute in one operation
pub struct OnboardingEnsureOp;

#[async_trait]
impl CustomOperation for OnboardingEnsureOp {
    fn domain(&self) -> &'static str {
        "onboarding"
    }
    fn verb(&self) -> &'static str {
        "ensure"
    }
    fn rationale(&self) -> &'static str {
        "Idempotent plan+execute requiring existing state check and conditional execution"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        let cbu_id = resolve_cbu_id(verb_call, ctx, pool).await?;
        let products = extract_string_list(verb_call, "products")?;

        // Normalize products for consistent comparison
        let normalized_products = normalize_products(&products);

        // Check for existing complete onboarding using array containment (order-independent)
        // products @> normalized AND products <@ normalized = set equality
        let existing: Option<(Uuid, Uuid)> = sqlx::query_as(
            r#"SELECT op.plan_id, oe.execution_id
               FROM "ob-poc".onboarding_plans op
               JOIN "ob-poc".onboarding_executions oe ON oe.plan_id = op.plan_id
               WHERE op.cbu_id = $1
                 AND op.products @> $2
                 AND op.products <@ $2
                 AND op.status = 'complete'
                 AND oe.status = 'complete'
               ORDER BY oe.completed_at DESC
               LIMIT 1"#,
        )
        .bind(cbu_id)
        .bind(&normalized_products)
        .fetch_optional(pool)
        .await?;

        if let Some((plan_id, execution_id)) = existing {
            // Already onboarded - return existing resource URLs
            let urls = collect_provisioned_urls(pool, execution_id).await?;

            return Ok(ExecutionResult::Record(serde_json::json!({
                "status": "already_complete",
                "plan_id": plan_id,
                "execution_id": execution_id,
                "urls": urls
            })));
        }

        // Create new plan
        let resources = expand_products_to_resources(pool, &products).await?;

        if resources.is_empty() {
            return Err(anyhow!("No resources found for products: {:?}", products));
        }

        let dep_graph = build_resource_dependency_graph(pool, &resources).await?;
        let dsl = generate_provisioning_dsl(&cbu_id, &resources, &dep_graph);
        let plan_id = store_onboarding_plan(pool, cbu_id, &products, &dsl, &dep_graph).await?;

        // Execute the plan
        let execution_id = Uuid::new_v4();
        sqlx::query(
            r#"INSERT INTO "ob-poc".onboarding_executions
               (execution_id, plan_id, status, started_at)
               VALUES ($1, $2, 'running', NOW())"#,
        )
        .bind(execution_id)
        .bind(plan_id)
        .execute(pool)
        .await?;

        sqlx::query(
            r#"UPDATE "ob-poc".onboarding_plans SET status = 'executing' WHERE plan_id = $1"#,
        )
        .bind(plan_id)
        .execute(pool)
        .await?;

        // Execute the generated DSL via the standard executor
        let executor = crate::dsl_v2::executor::DslExecutor::new(pool.clone());
        let exec_result = executor.execute_dsl(&dsl, ctx).await;

        // Update status
        match &exec_result {
            Ok(_) => {
                sqlx::query(
                    r#"UPDATE "ob-poc".onboarding_executions
                       SET status = 'complete', completed_at = NOW()
                       WHERE execution_id = $1"#,
                )
                .bind(execution_id)
                .execute(pool)
                .await?;

                sqlx::query(
                    r#"UPDATE "ob-poc".onboarding_plans SET status = 'complete' WHERE plan_id = $1"#,
                )
                .bind(plan_id)
                .execute(pool)
                .await?;
            }
            Err(e) => {
                sqlx::query(
                    r#"UPDATE "ob-poc".onboarding_executions
                       SET status = 'failed', completed_at = NOW(), error_message = $2
                       WHERE execution_id = $1"#,
                )
                .bind(execution_id)
                .bind(e.to_string())
                .execute(pool)
                .await?;

                sqlx::query(
                    r#"UPDATE "ob-poc".onboarding_plans SET status = 'failed' WHERE plan_id = $1"#,
                )
                .bind(plan_id)
                .execute(pool)
                .await?;

                return Err(anyhow!("Execution failed: {}", e));
            }
        }

        // Collect URLs - note: for now return empty since we don't track task→instance mapping yet
        // In production, the execute step would populate onboarding_tasks
        Ok(ExecutionResult::Record(serde_json::json!({
            "status": "complete",
            "plan_id": plan_id,
            "execution_id": execution_id,
            "urls": {}
        })))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Ok(ExecutionResult::Record(
            serde_json::json!({"status": "complete"}),
        ))
    }
}

/// onboarding.auto-complete - Automatically progress onboarding by generating DSL
///
/// This operation derives semantic state, finds missing entities, generates DSL
/// to create them, and optionally executes. It's an "auto-pilot" for onboarding.
pub struct OnboardingAutoCompleteOp;

/// Result of a single auto-complete step
#[derive(Debug, Clone, serde::Serialize)]
pub struct AutoCompleteStep {
    pub entity_type: String,
    pub stage: String,
    pub dsl: String,
    pub executed: bool,
    pub success: bool,
    pub error: Option<String>,
    pub created_id: Option<Uuid>,
}

/// Result of the auto-complete operation
#[derive(Debug, Clone, serde::Serialize)]
pub struct AutoCompleteResult {
    pub steps_executed: usize,
    pub steps_succeeded: usize,
    pub steps_failed: usize,
    pub steps: Vec<AutoCompleteStep>,
    pub remaining_missing: Vec<String>,
    pub target_reached: bool,
    pub dry_run: bool,
}

#[cfg(feature = "database")]
impl OnboardingAutoCompleteOp {
    /// Generate DSL for creating a missing entity
    fn generate_entity_dsl(
        cbu_id: Uuid,
        entity_type: &str,
        existing: &std::collections::HashMap<String, Vec<Uuid>>,
    ) -> Option<String> {
        match entity_type {
            "kyc_case" => Some(format!(
                r#"(kyc-case.create :cbu-id "{}" :case-type "NEW_CLIENT" :as @case)"#,
                cbu_id
            )),

            "entity_workstream" => {
                // Need a case_id - get from existing if available
                let case_id = existing.get("kyc_case").and_then(|ids| ids.first())?;
                // For workstream, we need an entity - use the CBU's first entity with a role
                // This is a placeholder - in production we'd query for entities needing workstreams
                Some(format!(
                    r#"; Entity workstream requires entity selection
(entity-workstream.create :case-id "{}" :entity-id <select-entity> :as @workstream)"#,
                    case_id
                ))
            }

            "trading_profile" => Some(format!(
                r#"(trading-profile.import :cbu-id "{}" :profile-path "config/seed/trading_profiles/default.yaml" :as @profile)"#,
                cbu_id
            )),

            "cbu_instrument_universe" => Some(format!(
                r#"(cbu-custody.add-universe :cbu-id "{}" :instrument-class "EQUITY" :market "XNYS" :currencies ["USD"] :settlement-types ["DVP"])"#,
                cbu_id
            )),

            "cbu_ssi" => Some(format!(
                r#"(cbu-custody.create-ssi :cbu-id "{}" :name "Default SSI" :type "SECURITIES" :safekeeping-account "SAFE-001" :safekeeping-bic "CUSTUS33" :cash-account "CASH-001" :cash-bic "CUSTUS33" :cash-currency "USD" :pset-bic "DTCYUS33" :effective-date "2024-01-01" :as @ssi)"#,
                cbu_id
            )),

            "ssi_booking_rule" => {
                let ssi_id = existing.get("cbu_ssi").and_then(|ids| ids.first())?;
                Some(format!(
                    r#"(cbu-custody.add-booking-rule :cbu-id "{}" :ssi-id "{}" :name "Default Rule" :priority 100)"#,
                    cbu_id, ssi_id
                ))
            }

            "isda_agreement" => Some(format!(
                r#"; ISDA requires counterparty selection
(isda.create :cbu-id "{}" :counterparty-id <select-counterparty> :governing-law "NY" :agreement-date "2024-01-01" :as @isda)"#,
                cbu_id
            )),

            "csa_agreement" => {
                let isda_id = existing.get("isda_agreement").and_then(|ids| ids.first())?;
                Some(format!(
                    r#"(isda.add-csa :isda-id "{}" :csa-type "VM" :threshold-amount 0 :minimum-transfer 500000 :as @csa)"#,
                    isda_id
                ))
            }

            "cbu_resource_instance" | "cbu_lifecycle_instance" => Some(format!(
                r#"(lifecycle.provision :cbu-id "{}" :lifecycle-code "CUSTODY_ONBOARD" :as @lifecycle)"#,
                cbu_id
            )),

            "cbu_pricing_config" => Some(format!(
                r#"(pricing-config.set :cbu-id "{}" :instrument-class "EQUITY" :source "BLOOMBERG" :priority 10)"#,
                cbu_id
            )),

            "share_class" => Some(format!(
                r#"(share-class.create :cbu-id "{}" :name "Class A" :currency "USD" :class-category "FUND" :as @share_class)"#,
                cbu_id
            )),

            "holding" => {
                let share_class_id = existing.get("share_class").and_then(|ids| ids.first())?;
                Some(format!(
                    r#"; Holding requires investor entity selection
(holding.create :share-class-id "{}" :investor-entity-id <select-investor> :as @holding)"#,
                    share_class_id
                ))
            }

            _ => None,
        }
    }
}

#[async_trait]
impl CustomOperation for OnboardingAutoCompleteOp {
    fn domain(&self) -> &'static str {
        "onboarding"
    }
    fn verb(&self) -> &'static str {
        "auto-complete"
    }
    fn rationale(&self) -> &'static str {
        "Requires semantic state derivation, DSL generation, and iterative execution"
    }

    #[cfg(feature = "database")]
    async fn execute(
        &self,
        verb_call: &VerbCall,
        ctx: &mut ExecutionContext,
        pool: &PgPool,
    ) -> Result<ExecutionResult> {
        use crate::database::derive_semantic_state;
        use crate::ontology::SemanticStageRegistry;

        // Extract arguments
        let cbu_id = extract_uuid(verb_call, ctx, "cbu-id")?;

        let max_steps: i32 = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "max-steps")
            .and_then(|a| a.value.as_integer())
            .unwrap_or(20) as i32;

        let dry_run: bool = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "dry-run")
            .and_then(|a| a.value.as_boolean())
            .unwrap_or(false);

        let target_stage: Option<String> = verb_call
            .arguments
            .iter()
            .find(|a| a.key == "target-stage")
            .and_then(|a| a.value.as_string().map(|s| s.to_string()));

        // Load semantic stage registry
        let registry = SemanticStageRegistry::load_default()
            .map_err(|e| anyhow!("Failed to load semantic stage registry: {}", e))?;

        let mut steps: Vec<AutoCompleteStep> = Vec::new();
        let mut steps_executed = 0;
        let mut steps_succeeded = 0;
        let mut steps_failed = 0;

        // Create executor for DSL execution
        let executor = crate::dsl_v2::executor::DslExecutor::new(pool.clone());

        // Iterative loop: derive state, find missing, generate DSL, execute
        for _ in 0..max_steps {
            // Derive current semantic state
            let state = derive_semantic_state(pool, &registry, cbu_id).await?;

            // Check if target stage is complete
            if let Some(ref target) = target_stage {
                let target_complete = state.required_stages.iter().any(|s| {
                    &s.code == target
                        && s.status == ob_poc_types::semantic_stage::StageStatus::Complete
                });
                if target_complete {
                    return Ok(ExecutionResult::Record(serde_json::to_value(
                        AutoCompleteResult {
                            steps_executed,
                            steps_succeeded,
                            steps_failed,
                            steps,
                            remaining_missing: vec![],
                            target_reached: true,
                            dry_run,
                        },
                    )?));
                }
            }

            // Find next missing entity to create
            if state.missing_entities.is_empty() {
                // No more missing entities - we're done
                break;
            }

            // Get the first actionable missing entity
            // Prioritize by stage order (stages that are unblocked first)
            let next_missing = state
                .missing_entities
                .iter()
                .find(|m| state.next_actionable.contains(&m.stage));

            let missing = match next_missing {
                Some(m) => m,
                None => {
                    // No actionable missing entities - might be blocked
                    break;
                }
            };

            // Build existing entities map for DSL generation
            let existing: std::collections::HashMap<String, Vec<Uuid>> = state
                .required_stages
                .iter()
                .flat_map(|s| &s.required_entities)
                .filter(|e| e.exists)
                .map(|e| (e.entity_type.clone(), e.ids.clone()))
                .collect();

            // Generate DSL for this entity
            let dsl = match Self::generate_entity_dsl(cbu_id, &missing.entity_type, &existing) {
                Some(d) => d,
                None => {
                    // Can't generate DSL for this entity type
                    steps.push(AutoCompleteStep {
                        entity_type: missing.entity_type.clone(),
                        stage: missing.stage.clone(),
                        dsl: String::new(),
                        executed: false,
                        success: false,
                        error: Some(format!(
                            "No DSL template for entity type: {}",
                            missing.entity_type
                        )),
                        created_id: None,
                    });
                    steps_failed += 1;
                    continue;
                }
            };

            // Check if DSL contains placeholders that need user input
            if dsl.contains("<select-") {
                steps.push(AutoCompleteStep {
                    entity_type: missing.entity_type.clone(),
                    stage: missing.stage.clone(),
                    dsl: dsl.clone(),
                    executed: false,
                    success: false,
                    error: Some("DSL requires user selection - cannot auto-complete".to_string()),
                    created_id: None,
                });
                // Don't count as failed - it's expected that some entities need user input
                break;
            }

            if dry_run {
                // In dry-run mode, just collect the DSL without executing
                steps.push(AutoCompleteStep {
                    entity_type: missing.entity_type.clone(),
                    stage: missing.stage.clone(),
                    dsl: dsl.clone(),
                    executed: false,
                    success: true,
                    error: None,
                    created_id: None,
                });
                steps_executed += 1;
                steps_succeeded += 1;
            } else {
                // Execute the DSL
                steps_executed += 1;
                let result = executor.execute_dsl(&dsl, ctx).await;

                match result {
                    Ok(_) => {
                        steps_succeeded += 1;
                        steps.push(AutoCompleteStep {
                            entity_type: missing.entity_type.clone(),
                            stage: missing.stage.clone(),
                            dsl: dsl.clone(),
                            executed: true,
                            success: true,
                            error: None,
                            created_id: None, // Could extract from ctx bindings
                        });
                    }
                    Err(e) => {
                        steps_failed += 1;
                        steps.push(AutoCompleteStep {
                            entity_type: missing.entity_type.clone(),
                            stage: missing.stage.clone(),
                            dsl: dsl.clone(),
                            executed: true,
                            success: false,
                            error: Some(e.to_string()),
                            created_id: None,
                        });
                        // Stop on first error to avoid cascading failures
                        break;
                    }
                }
            }
        }

        // Get final state to report remaining missing entities
        let final_state = derive_semantic_state(pool, &registry, cbu_id).await?;
        let remaining_missing: Vec<String> = final_state
            .missing_entities
            .iter()
            .map(|m| format!("{} ({})", m.entity_type, m.stage))
            .collect();

        let target_reached = if let Some(ref target) = target_stage {
            final_state.required_stages.iter().any(|s| {
                &s.code == target && s.status == ob_poc_types::semantic_stage::StageStatus::Complete
            })
        } else {
            remaining_missing.is_empty()
        };

        Ok(ExecutionResult::Record(serde_json::to_value(
            AutoCompleteResult {
                steps_executed,
                steps_succeeded,
                steps_failed,
                steps,
                remaining_missing,
                target_reached,
                dry_run,
            },
        )?))
    }

    #[cfg(not(feature = "database"))]
    async fn execute(
        &self,
        _verb_call: &VerbCall,
        _ctx: &mut ExecutionContext,
    ) -> Result<ExecutionResult> {
        Err(anyhow!(
            "onboarding.auto-complete requires database feature"
        ))
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_graph_topo_order() {
        let mut graph = ResourceDependencyGraph::new();

        let id1 = Uuid::new_v4(); // SETTLE_ACCT (no deps)
        let id2 = Uuid::new_v4(); // CUSTODY_ACCT (depends on SETTLE_ACCT)
        let id3 = Uuid::new_v4(); // SWIFT_CONN (depends on CUSTODY_ACCT)
        let id4 = Uuid::new_v4(); // CA_PLATFORM (depends on CUSTODY_ACCT)

        graph.add_node(id1, "SETTLE_ACCT".to_string());
        graph.add_node(id2, "CUSTODY_ACCT".to_string());
        graph.add_node(id3, "SWIFT_CONN".to_string());
        graph.add_node(id4, "CA_PLATFORM".to_string());

        // custody depends on settle
        graph.add_edge(
            id2,
            id1,
            "required".to_string(),
            "settlement-account-url".to_string(),
        );
        // swift depends on custody
        graph.add_edge(
            id3,
            id2,
            "required".to_string(),
            "custody-account-url".to_string(),
        );
        // ca_platform depends on custody
        graph.add_edge(
            id4,
            id2,
            "required".to_string(),
            "custody-account-url".to_string(),
        );

        let stages = graph.compute_stages().unwrap();

        // Stage 0: SETTLE_ACCT (no deps)
        // Stage 1: CUSTODY_ACCT (depends on stage 0)
        // Stage 2: SWIFT_CONN, CA_PLATFORM (both depend on stage 1, parallel)
        assert_eq!(stages.len(), 3);
        assert!(stages[0].contains(&id1));
        assert!(stages[1].contains(&id2));
        assert!(stages[2].contains(&id3));
        assert!(stages[2].contains(&id4));
    }

    #[test]
    fn test_normalize_products() {
        let products1 = vec!["CUSTODY".to_string(), "FUND_ACCOUNTING".to_string()];
        let products2 = vec!["FUND_ACCOUNTING".to_string(), "CUSTODY".to_string()];

        assert_eq!(
            normalize_products(&products1),
            normalize_products(&products2)
        );
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = ResourceDependencyGraph::new();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        graph.add_node(id1, "A".to_string());
        graph.add_node(id2, "B".to_string());

        // A depends on B
        graph.add_edge(id1, id2, "required".to_string(), "b-url".to_string());
        // B depends on A (cycle!)
        graph.add_edge(id2, id1, "required".to_string(), "a-url".to_string());

        let result = graph.compute_stages();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cycle"));
    }
}
