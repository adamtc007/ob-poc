# TODO: Unified Entity Dependency DAG

## Objective

Single config-driven dependency model that powers:
1. **Compiler** - topological sort for execution ordering
2. **Linter/LSP** - validation of entity references and type constraints
3. **Auto-complete** - suggest valid entity types for arguments

**Two config points only:**
- `verbs/*.yaml` - what verbs produce
- `entity_type_dependencies` table - what types depend on what

**Zero code changes** for new verbs, entities, or resources.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              CONFIG LAYER                                    │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  verbs/*.yaml                          entity_type_dependencies              │
│  ┌────────────────────────┐            ┌─────────────────────────────────┐  │
│  │ produces:              │            │ from_type    │ to_type          │  │
│  │   type: entity         │            │ from_subtype │ to_subtype       │  │
│  │   subtype: fund_sub    │            │ via_arg      │ dependency_kind  │  │
│  │   subtype_from_arg: .. │            │──────────────┼──────────────────│  │
│  │                        │            │ entity/fund_sub → fund_umbrella │  │
│  │ consumes:              │            │ resource/CUSTODY → SETTLE       │  │
│  │   - arg: umbrella-id   │            │ case → cbu                      │  │
│  │     type: entity       │            └─────────────────────────────────┘  │
│  │     subtype: umbrella  │                                                  │
│  └────────────────────────┘                                                  │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
                                       │
                     ┌─────────────────┴─────────────────┐
                     ▼                                   ▼
          ┌─────────────────────┐             ┌─────────────────────┐
          │      Compiler       │             │    Linter / LSP     │
          │    (topo_sort.rs)   │             │  (lsp_validator.rs) │
          ├─────────────────────┤             ├─────────────────────┤
          │ • Execution order   │             │ • Type validation   │
          │ • Cycle detection   │             │ • Missing deps      │
          │ • Parallel stages   │             │ • Auto-complete     │
          └─────────────────────┘             └─────────────────────┘
```

---

## Phase 1: Schema

### 1.1 Create `entity_type_dependencies` Table

```sql
-- Unified dependency config for ALL entity types
-- This replaces the resource_dependencies table with a generalized model
CREATE TABLE "ob-poc".entity_type_dependencies (
    dependency_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    
    -- Source: what type/subtype has the dependency
    from_type VARCHAR(50) NOT NULL,
    from_subtype VARCHAR(50),
    
    -- Target: what it depends on
    to_type VARCHAR(50) NOT NULL,
    to_subtype VARCHAR(50),
    
    -- How the dependency is expressed in DSL args
    via_arg VARCHAR(100),
    
    -- Dependency characteristics
    dependency_kind VARCHAR(20) DEFAULT 'required'
        CHECK (dependency_kind IN ('required', 'optional', 'conditional')),
    
    -- For conditional dependencies
    condition_expr TEXT,
    
    -- Ordering hint for same-level dependencies
    priority INTEGER DEFAULT 100,
    
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now(),
    
    UNIQUE(from_type, from_subtype, to_type, to_subtype)
);

CREATE INDEX idx_entity_deps_from 
    ON "ob-poc".entity_type_dependencies(from_type, from_subtype);
CREATE INDEX idx_entity_deps_to 
    ON "ob-poc".entity_type_dependencies(to_type, to_subtype);

COMMENT ON TABLE "ob-poc".entity_type_dependencies IS 
'Unified entity/resource dependency graph. Drives compiler ordering and linter validation.
from_type/subtype depends on to_type/subtype. via_arg indicates which verb argument 
carries the reference (for linter validation).';
```

### 1.2 Seed Core Dependencies

```sql
-- =============================================================================
-- STRUCTURAL DEPENDENCIES (type-level)
-- =============================================================================

INSERT INTO "ob-poc".entity_type_dependencies 
(from_type, from_subtype, to_type, to_subtype, via_arg, dependency_kind) VALUES

-- Case/Workstream hierarchy
('case', NULL, 'cbu', NULL, 'cbu-id', 'required'),
('workstream', NULL, 'case', NULL, 'case-id', 'required'),

-- Documents attach to entities
('document', NULL, 'entity', NULL, 'entity-id', 'optional'),
('document', NULL, 'cbu', NULL, 'cbu-id', 'optional'),

-- Observations attach to entities
('observation', NULL, 'entity', NULL, 'entity-id', 'required');

-- =============================================================================
-- FUND HIERARCHY (subtype-level)
-- =============================================================================

INSERT INTO "ob-poc".entity_type_dependencies 
(from_type, from_subtype, to_type, to_subtype, via_arg, dependency_kind) VALUES

-- Umbrella needs a legal entity
('entity', 'fund_umbrella', 'entity', NULL, 'legal-entity-id', 'required'),

-- Sub-fund needs umbrella
('entity', 'fund_sub', 'entity', 'fund_umbrella', 'umbrella-id', 'required'),

-- Share class needs sub-fund
('entity', 'share_class', 'entity', 'fund_sub', 'sub-fund-id', 'required'),

-- Master-feeder structure
('entity', 'fund_master', 'entity', 'fund_umbrella', 'umbrella-id', 'required'),
('entity', 'fund_feeder', 'entity', 'fund_master', 'master-fund-id', 'required');

-- =============================================================================
-- SERVICE RESOURCES (subtype = resource_code)
-- =============================================================================

INSERT INTO "ob-poc".entity_type_dependencies 
(from_type, from_subtype, to_type, to_subtype, via_arg, dependency_kind) VALUES

-- Settlement account is root (no deps)
-- Custody depends on settlement
('resource_instance', 'CUSTODY_ACCT', 'resource_instance', 'SETTLE_ACCT', NULL, 'required'),

-- These depend on custody
('resource_instance', 'SWIFT_CONN', 'resource_instance', 'CUSTODY_ACCT', NULL, 'required'),
('resource_instance', 'NAV_ENGINE', 'resource_instance', 'CUSTODY_ACCT', NULL, 'required'),
('resource_instance', 'CA_PLATFORM', 'resource_instance', 'CUSTODY_ACCT', NULL, 'required'),
('resource_instance', 'REPORTING', 'resource_instance', 'CUSTODY_ACCT', NULL, 'required');
```

### 1.3 Migration: Deprecate `resource_dependencies`

```sql
-- After entity_type_dependencies is populated and tested:
-- 1. Verify all resource deps are migrated
SELECT rd.*, srt1.resource_code as from_code, srt2.resource_code as to_code
FROM "ob-poc".resource_dependencies rd
JOIN "ob-poc".service_resource_types srt1 ON srt1.resource_id = rd.resource_type_id
JOIN "ob-poc".service_resource_types srt2 ON srt2.resource_id = rd.depends_on_type_id
WHERE NOT EXISTS (
    SELECT 1 FROM "ob-poc".entity_type_dependencies etd
    WHERE etd.from_type = 'resource_instance'
      AND etd.from_subtype = srt1.resource_code
      AND etd.to_type = 'resource_instance'
      AND etd.to_subtype = srt2.resource_code
);

-- 2. Drop old table (after verification)
-- DROP TABLE "ob-poc".resource_dependencies;
```

---

## Phase 2: Config Types Extension

### 2.1 Extend `VerbProduces`

File: `rust/src/dsl_v2/config/types.rs`

```rust
/// Dataflow: what a verb produces when executed with :as @binding
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbProduces {
    /// The type of entity produced: "cbu", "entity", "case", "resource_instance", etc.
    #[serde(rename = "type")]
    pub produced_type: String,
    
    /// Static subtype: "fund_umbrella", "proper_person", etc.
    #[serde(default)]
    pub subtype: Option<String>,
    
    /// Dynamic subtype from argument value
    /// e.g., "resource-type" arg contains "CUSTODY_ACCT"
    #[serde(default)]
    pub subtype_from_arg: Option<String>,
    
    /// True if this is a lookup (resolved existing) rather than create (new)
    #[serde(default)]
    pub resolved: bool,
    
    /// Initial state when creating a new entity (for lifecycle tracking)
    #[serde(default)]
    pub initial_state: Option<String>,
}

impl VerbProduces {
    /// Extract the subtype for a given verb call
    /// Returns static subtype, or extracts from arg if subtype_from_arg is set
    pub fn resolve_subtype(&self, args: &[Argument]) -> Option<String> {
        // Static subtype takes precedence
        if let Some(ref st) = self.subtype {
            return Some(st.clone());
        }
        
        // Dynamic subtype from arg
        if let Some(ref arg_name) = self.subtype_from_arg {
            return args.iter()
                .find(|a| a.key == *arg_name)
                .and_then(|a| a.value.as_string())
                .map(|s| s.to_string());
        }
        
        None
    }
}
```

### 2.2 Update Verb YAML Files

File: `rust/config/verbs/service-resource.yaml`

```yaml
provision:
  description: Provision a service resource instance for a CBU
  behavior: plugin
  handler: resource_instance_create
  produces:
    type: resource_instance
    subtype_from_arg: resource-type    # <-- NEW: dynamic subtype
  args:
    - name: cbu-id
      type: uuid
      required: true
    - name: resource-type
      type: string
      required: true
    # ... rest unchanged
```

File: `rust/config/verbs/fund.yaml`

```yaml
create-umbrella:
  description: Create an umbrella fund structure
  behavior: crud
  produces:
    type: entity
    subtype: fund_umbrella              # <-- static subtype
  consumes:
    - arg: legal-entity-id
      type: entity                      # <-- type constraint for linter
  args:
    - name: legal-entity-id
      type: uuid
      required: true
    # ...

create-sub-fund:
  description: Create a sub-fund under an umbrella
  behavior: crud
  produces:
    type: entity
    subtype: fund_sub
  consumes:
    - arg: umbrella-id
      type: entity
      subtype: fund_umbrella            # <-- subtype constraint for linter
  args:
    - name: umbrella-id
      type: uuid
      required: true
    # ...
```

---

## Phase 3: Entity Dependency Registry

### 3.1 Create Registry Module

File: `rust/src/dsl_v2/entity_deps.rs`

```rust
//! Entity Type Dependency Registry
//!
//! Loads dependency graph from entity_type_dependencies table.
//! Used by compiler (topo_sort) and linter (validation).

use anyhow::Result;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

#[cfg(feature = "database")]
use sqlx::PgPool;

/// Dependency between entity types
#[derive(Debug, Clone)]
pub struct EntityDep {
    pub to_type: String,
    pub to_subtype: Option<String>,
    pub via_arg: Option<String>,
    pub kind: DependencyKind,
    pub priority: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyKind {
    Required,
    Optional,
    Conditional,
}

/// Registry of entity type dependencies
/// Loaded once at startup, shared across compiler and linter
#[derive(Debug, Clone)]
pub struct EntityDependencyRegistry {
    /// (from_type, from_subtype) -> Vec<EntityDep>
    deps: HashMap<TypeKey, Vec<EntityDep>>,
}

type TypeKey = (String, Option<String>);

impl EntityDependencyRegistry {
    /// Create empty registry (for testing)
    pub fn empty() -> Self {
        Self { deps: HashMap::new() }
    }
    
    /// Load from database
    #[cfg(feature = "database")]
    pub async fn load(pool: &PgPool) -> Result<Self> {
        let rows: Vec<(String, Option<String>, String, Option<String>, Option<String>, String, i32)> = 
            sqlx::query_as(
                r#"SELECT from_type, from_subtype, to_type, to_subtype, 
                          via_arg, dependency_kind, priority
                   FROM "ob-poc".entity_type_dependencies
                   WHERE is_active = true
                   ORDER BY from_type, from_subtype, priority"#
            )
            .fetch_all(pool)
            .await?;
        
        let mut deps: HashMap<TypeKey, Vec<EntityDep>> = HashMap::new();
        
        for (from_type, from_subtype, to_type, to_subtype, via_arg, kind, priority) in rows {
            let key = (from_type, from_subtype);
            let dep = EntityDep {
                to_type,
                to_subtype,
                via_arg,
                kind: match kind.as_str() {
                    "optional" => DependencyKind::Optional,
                    "conditional" => DependencyKind::Conditional,
                    _ => DependencyKind::Required,
                },
                priority,
            };
            deps.entry(key).or_default().push(dep);
        }
        
        Ok(Self { deps })
    }
    
    /// Get dependencies for a type/subtype
    /// Checks both exact match and type-only match
    pub fn dependencies_of(&self, from_type: &str, from_subtype: Option<&str>) -> Vec<&EntityDep> {
        let mut result = Vec::new();
        
        // Exact match (type + subtype)
        if let Some(subtype) = from_subtype {
            let key = (from_type.to_string(), Some(subtype.to_string()));
            if let Some(deps) = self.deps.get(&key) {
                result.extend(deps.iter());
            }
        }
        
        // Type-only match (applies to all subtypes)
        let key = (from_type.to_string(), None);
        if let Some(deps) = self.deps.get(&key) {
            result.extend(deps.iter());
        }
        
        result
    }
    
    /// Check if a type/subtype has any dependencies
    pub fn has_dependencies(&self, from_type: &str, from_subtype: Option<&str>) -> bool {
        !self.dependencies_of(from_type, from_subtype).is_empty()
    }
    
    /// Get all known type/subtype pairs (for validation)
    pub fn known_types(&self) -> impl Iterator<Item = (&str, Option<&str>)> {
        self.deps.keys().map(|(t, st)| (t.as_str(), st.as_deref()))
    }
    
    /// Validate that a dependency target exists in our registry
    /// Returns true if to_type/to_subtype is known as a "from" somewhere
    /// (meaning something produces it)
    pub fn is_known_type(&self, check_type: &str, check_subtype: Option<&str>) -> bool {
        // A type is "known" if it appears as a from_type somewhere,
        // or if it's a base type (cbu, entity, etc.)
        let base_types = ["cbu", "entity", "case", "workstream", "document", 
                         "observation", "resource_instance"];
        
        if base_types.contains(&check_type) && check_subtype.is_none() {
            return true;
        }
        
        self.deps.contains_key(&(check_type.to_string(), check_subtype.map(|s| s.to_string())))
    }
}

// =============================================================================
// GLOBAL REGISTRY (lazy loaded)
// =============================================================================

static ENTITY_DEPS: OnceLock<Arc<EntityDependencyRegistry>> = OnceLock::new();

/// Get the global entity dependency registry
pub fn entity_deps() -> &'static Arc<EntityDependencyRegistry> {
    ENTITY_DEPS.get_or_init(|| {
        // In production, this would be loaded from DB at startup
        // For now, return empty registry (tests will inject their own)
        Arc::new(EntityDependencyRegistry::empty())
    })
}

/// Initialize global registry from database (call once at startup)
#[cfg(feature = "database")]
pub async fn init_entity_deps(pool: &PgPool) -> Result<()> {
    let registry = EntityDependencyRegistry::load(pool).await?;
    let _ = ENTITY_DEPS.set(Arc::new(registry));
    Ok(())
}

/// Set registry for testing
#[cfg(test)]
pub fn set_entity_deps_for_test(registry: EntityDependencyRegistry) {
    let _ = ENTITY_DEPS.set(Arc::new(registry));
}
```

### 3.2 Register Module

File: `rust/src/dsl_v2/mod.rs`

```rust
pub mod entity_deps;

pub use entity_deps::{
    EntityDependencyRegistry, EntityDep, DependencyKind,
    entity_deps, init_entity_deps,
};
```

---

## Phase 4: Compiler Integration (topo_sort.rs)

### 4.1 Unified Topological Sort

Replace the multiple topo sort implementations with a single unified version.

File: `rust/src/dsl_v2/topo_sort.rs`

```rust
use super::entity_deps::{EntityDependencyRegistry, DependencyKind};

/// Unified topological sort with entity dependency awareness
///
/// Builds execution DAG from:
/// 1. Explicit @symbol references
/// 2. Entity type dependencies (from entity_type_dependencies table)
/// 3. Lifecycle state requirements
pub fn topological_sort_unified(
    pending: &Program,
    executed_context: &BindingContext,
    entity_deps: &EntityDependencyRegistry,
    verb_registry: &RuntimeVerbRegistry,
) -> Result<TopoSortResult, TopoSortError> {
    let statements = &pending.statements;
    
    if statements.is_empty() {
        return Ok(TopoSortResult {
            program: pending.clone(),
            reordered: false,
            index_map: vec![],
            lifecycle_diagnostics: vec![],
        });
    }
    
    let mut deps: HashMap<usize, HashSet<usize>> = HashMap::new();
    
    // Index: @binding -> stmt idx
    let mut binding_to_stmt: HashMap<String, usize> = HashMap::new();
    
    // Index: (type, subtype) -> [stmt indices]
    let mut type_producers: HashMap<(String, Option<String>), Vec<usize>> = HashMap::new();
    
    // =========================================================================
    // PASS 1: Record what each statement produces
    // =========================================================================
    for (idx, stmt) in statements.iter().enumerate() {
        deps.insert(idx, HashSet::new());
        
        if let Statement::VerbCall(vc) = stmt {
            // Track @binding
            if let Some(ref binding) = vc.binding {
                binding_to_stmt.insert(binding.clone(), idx);
            }
            
            // Track (type, subtype) production
            if let Some((prod_type, prod_subtype)) = extract_produced_type(vc, verb_registry) {
                // Index by exact type+subtype
                type_producers
                    .entry((prod_type.clone(), prod_subtype.clone()))
                    .or_default()
                    .push(idx);
                
                // Also index by type-only (for type-level deps)
                if prod_subtype.is_some() {
                    type_producers
                        .entry((prod_type, None))
                        .or_default()
                        .push(idx);
                }
            }
        }
    }
    
    // =========================================================================
    // PASS 2: Build dependency edges
    // =========================================================================
    for (idx, stmt) in statements.iter().enumerate() {
        if let Statement::VerbCall(vc) = stmt {
            
            // -----------------------------------------------------------------
            // (A) Explicit @symbol references
            // -----------------------------------------------------------------
            for arg in &vc.arguments {
                collect_symbol_refs(
                    &arg.value,
                    &binding_to_stmt,
                    executed_context,
                    idx,
                    &mut deps,
                );
            }
            
            // -----------------------------------------------------------------
            // (B) Entity type dependencies (config-driven)
            // -----------------------------------------------------------------
            if let Some((prod_type, prod_subtype)) = extract_produced_type(vc, verb_registry) {
                for entity_dep in entity_deps.dependencies_of(&prod_type, prod_subtype.as_deref()) {
                    // Find statements that produce the dependency type
                    let dep_key = (entity_dep.to_type.clone(), entity_dep.to_subtype.clone());
                    
                    if let Some(producer_indices) = type_producers.get(&dep_key) {
                        for &producer_idx in producer_indices {
                            if producer_idx != idx {
                                deps.get_mut(&idx).unwrap().insert(producer_idx);
                            }
                        }
                    }
                    
                    // If subtype-specific lookup failed, try type-only
                    if entity_dep.to_subtype.is_some() {
                        let fallback_key = (entity_dep.to_type.clone(), None);
                        if let Some(producer_indices) = type_producers.get(&fallback_key) {
                            for &producer_idx in producer_indices {
                                if producer_idx != idx {
                                    deps.get_mut(&idx).unwrap().insert(producer_idx);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    // =========================================================================
    // PASS 3: Kahn's algorithm
    // =========================================================================
    kahn_topological_sort(&deps, statements)
}

/// Extract (type, subtype) that a verb call produces
fn extract_produced_type(
    vc: &VerbCall,
    registry: &RuntimeVerbRegistry,
) -> Option<(String, Option<String>)> {
    let verb_def = registry.get(&vc.domain, &vc.verb)?;
    let produces = verb_def.produces.as_ref()?;
    
    let prod_type = produces.produced_type.clone();
    let prod_subtype = produces.resolve_subtype(&vc.arguments);
    
    Some((prod_type, prod_subtype))
}

/// Kahn's algorithm for topological sort
fn kahn_topological_sort(
    deps: &HashMap<usize, HashSet<usize>>,
    statements: &[Statement],
) -> Result<TopoSortResult, TopoSortError> {
    let n = statements.len();
    
    // Calculate in-degrees
    let mut in_degree: HashMap<usize, usize> = HashMap::new();
    for idx in 0..n {
        in_degree.insert(idx, deps[&idx].len());
    }
    
    // Start with nodes that have no dependencies
    let mut queue: VecDeque<usize> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&idx, _)| idx)
        .collect();
    
    // Sort for stable ordering
    let mut queue_vec: Vec<usize> = queue.drain(..).collect();
    queue_vec.sort();
    queue = queue_vec.into_iter().collect();
    
    let mut sorted_indices = Vec::with_capacity(n);
    
    while let Some(idx) = queue.pop_front() {
        sorted_indices.push(idx);
        
        // Reduce in-degree of dependents
        let mut next_ready = Vec::new();
        for (other_idx, other_deps) in deps.iter() {
            if other_deps.contains(&idx) {
                let deg = in_degree.get_mut(other_idx).unwrap();
                *deg -= 1;
                if *deg == 0 {
                    next_ready.push(*other_idx);
                }
            }
        }
        
        next_ready.sort();
        for ready_idx in next_ready {
            queue.push_back(ready_idx);
        }
    }
    
    // Check for cycles
    if sorted_indices.len() != n {
        let cycle: Vec<String> = (0..n)
            .filter(|i| !sorted_indices.contains(i))
            .filter_map(|i| {
                if let Statement::VerbCall(vc) = &statements[i] {
                    Some(format!("{}.{}", vc.domain, vc.verb))
                } else {
                    None
                }
            })
            .collect();
        return Err(TopoSortError::CyclicDependency { cycle });
    }
    
    let reordered = sorted_indices.iter().enumerate().any(|(new, &old)| new != old);
    
    let sorted_statements: Vec<Statement> = sorted_indices
        .iter()
        .map(|&idx| statements[idx].clone())
        .collect();
    
    Ok(TopoSortResult {
        program: Program { statements: sorted_statements },
        reordered,
        index_map: sorted_indices,
        lifecycle_diagnostics: vec![],
    })
}
```

### 4.2 Wire Into Execution Pipeline

File: `rust/src/dsl_v2/execution_plan.rs`

```rust
use super::entity_deps::entity_deps;
use super::topo_sort::topological_sort_unified;

pub fn compile(program: &Program) -> Result<ExecutionPlan, CompileError> {
    // Use unified topo sort with entity dependencies
    let sorted = topological_sort_unified(
        program,
        &BindingContext::new(),
        entity_deps(),
        &runtime_registry(),
    ).map_err(|e| CompileError::SortError(e.to_string()))?;
    
    // Rest of compilation unchanged...
    compile_sorted(&sorted.program)
}
```

---

## Phase 5: Entity Resolution / Lookup Integration

The entity dependency model must integrate with the existing entity resolution system:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        ENTITY RESOLUTION FLOW                                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  DSL Argument                                                                │
│  :umbrella-id @umbrella                                                      │
│        │                                                                     │
│        ▼                                                                     │
│  ┌─────────────────┐     ┌──────────────────────┐                           │
│  │  LookupConfig   │────▶│    RefResolver       │                           │
│  │  (from YAML)    │     │  (GatewayRefResolver)│                           │
│  │                 │     └──────────┬───────────┘                           │
│  │  entity_type:   │                │                                        │
│  │    fund_umbrella│                ▼                                        │
│  │  search_key:    │     ┌──────────────────────┐                           │
│  │    name         │     │   EntityGateway      │                           │
│  │  primary_key:   │     │   (gRPC lookup)      │                           │
│  │    entity_id    │     └──────────────────────┘                           │
│  └─────────────────┘                                                         │
│                                                                              │
│  NEW: Entity dependency validation flows through same path                   │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐    │
│  │                  EntityDependencyRegistry                            │    │
│  │  • Validates type constraints from consumes                          │    │
│  │  • Checks dependency satisfaction                                    │    │
│  │  • Provides subtype suggestions                                      │    │
│  └─────────────────────────────────────────────────────────────────────┘    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### 5.1 Extend RefType Enum

The `RefType` enum needs to support subtypes for entity validation.

File: `rust/src/dsl_v2/validation.rs`

```rust
/// Reference types that map to DB tables
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefType {
    Cbu,
    Entity,
    Document,
    DocumentType,
    AttributeId,
    Jurisdiction,
    Role,
    EntityType,
    ScreeningType,
    Product,
    Service,
    Currency,
    ClientType,
    ResourceInstance,  // NEW: for service resources
}

/// Extended type info including subtype constraints
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeConstraint {
    pub ref_type: RefType,
    pub subtype: Option<String>,  // e.g., "fund_umbrella", "CUSTODY_ACCT"
}

impl TypeConstraint {
    pub fn new(ref_type: RefType) -> Self {
        Self { ref_type, subtype: None }
    }
    
    pub fn with_subtype(ref_type: RefType, subtype: impl Into<String>) -> Self {
        Self { ref_type, subtype: Some(subtype.into()) }
    }
    
    /// Check if this constraint is satisfied by a binding
    pub fn satisfied_by(&self, binding: &BindingInfo) -> bool {
        if self.ref_type != binding.ref_type {
            return false;
        }
        
        // If we require a subtype, binding must match
        if let Some(ref required_subtype) = self.subtype {
            match &binding.subtype {
                Some(actual) => actual == required_subtype,
                None => false,
            }
        } else {
            true // No subtype constraint, type match is enough
        }
    }
}

/// Info about a symbol binding - EXTENDED
#[derive(Debug, Clone)]
pub struct BindingInfo {
    pub name: String,
    pub ref_type: RefType,
    pub subtype: Option<String>,  // NEW: "fund_umbrella", "CUSTODY_ACCT", etc.
    pub defined_at: SourceSpan,
}
```

### 5.2 Extend LookupConfig for Subtype

File: `rust/src/dsl_v2/config/types.rs`

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LookupConfig {
    pub table: String,
    
    #[serde(default)]
    pub schema: Option<String>,
    
    /// The entity type for this lookup (maps to RefType)
    /// e.g., "entity", "cbu", "resource_instance"
    #[serde(default)]
    pub entity_type: Option<String>,
    
    /// NEW: Expected subtype for this lookup
    /// e.g., "fund_umbrella", "proper_person", "CUSTODY_ACCT"
    #[serde(default)]
    pub entity_subtype: Option<String>,
    
    #[serde(alias = "code_column")]
    pub search_key: SearchKeyConfig,
    
    pub primary_key: String,
}

impl LookupConfig {
    /// Convert to TypeConstraint for validation
    pub fn to_type_constraint(&self) -> Option<TypeConstraint> {
        let ref_type = self.entity_type.as_ref().and_then(|t| match t.as_str() {
            "cbu" => Some(RefType::Cbu),
            "entity" => Some(RefType::Entity),
            "document" => Some(RefType::Document),
            "resource_instance" => Some(RefType::ResourceInstance),
            // ... other mappings
            _ => None,
        })?;
        
        Some(TypeConstraint {
            ref_type,
            subtype: self.entity_subtype.clone(),
        })
    }
}
```

### 5.3 Update Verb YAML with Subtype Constraints

File: `rust/config/verbs/fund.yaml`

```yaml
create-sub-fund:
  description: Create a sub-fund under an umbrella
  behavior: crud
  produces:
    type: entity
    subtype: fund_sub
  args:
    - name: umbrella-id
      type: uuid
      required: true
      lookup:
        table: entities
        schema: ob-poc
        entity_type: entity
        entity_subtype: fund_umbrella    # NEW: subtype constraint
        search_key: name
        primary_key: entity_id
```

File: `rust/config/verbs/service-resource.yaml`

```yaml
provision:
  produces:
    type: resource_instance
    subtype_from_arg: resource-type
  args:
    - name: depends-on
      type: string_list
      required: false
      lookup:
        table: cbu_resource_instances
        schema: ob-poc
        entity_type: resource_instance
        # No entity_subtype - accepts any resource type
        # Dependency validation handled by entity_type_dependencies
        search_key: instance_url
        primary_key: instance_id
```

### 5.4 Integrate into GatewayRefResolver

File: `rust/src/dsl_v2/gateway_resolver.rs`

```rust
use super::entity_deps::{entity_deps, EntityDependencyRegistry};
use super::validation::{TypeConstraint, BindingInfo};

impl GatewayRefResolver {
    /// Resolve with type constraint validation
    pub async fn resolve_with_constraint(
        &mut self,
        constraint: &TypeConstraint,
        value: &str,
    ) -> Result<ResolveResult, String> {
        // First, do the basic lookup
        let result = self.resolve(constraint.ref_type, value).await?;
        
        // If found and we have a subtype constraint, validate it
        if let ResolveResult::Found { id, display } = &result {
            if let Some(ref required_subtype) = constraint.subtype {
                // Query the actual subtype of the found entity
                let actual_subtype = self.get_entity_subtype(constraint.ref_type, id).await?;
                
                if actual_subtype.as_ref() != Some(required_subtype) {
                    return Ok(ResolveResult::TypeMismatch {
                        id: *id,
                        display: display.clone(),
                        expected_subtype: required_subtype.clone(),
                        actual_subtype,
                    });
                }
            }
        }
        
        Ok(result)
    }
    
    /// Get the subtype of an entity (for validation)
    async fn get_entity_subtype(
        &mut self,
        ref_type: RefType,
        id: &Uuid,
    ) -> Result<Option<String>, String> {
        match ref_type {
            RefType::Entity => {
                // Query entity_types.type_code via entity.entity_type_id
                // This would be a Gateway call or direct query
                self.query_entity_subtype(id).await
            }
            RefType::ResourceInstance => {
                // Query service_resource_types.resource_code via instance.resource_type_id
                self.query_resource_subtype(id).await
            }
            _ => Ok(None), // Other types don't have subtypes
        }
    }
}

/// Extended resolve result
#[derive(Debug, Clone)]
pub enum ResolveResult {
    Found { id: Uuid, display: String },
    FoundByCode { code: String, uuid: Option<Uuid>, display: String },
    NotFound { suggestions: Vec<SuggestedMatch> },
    
    /// NEW: Found but wrong subtype
    TypeMismatch {
        id: Uuid,
        display: String,
        expected_subtype: String,
        actual_subtype: Option<String>,
    },
}
```

### 5.5 Entity Dependency Validation in Resolver

The ref resolver should check entity_type_dependencies when validating references.

File: `rust/src/dsl_v2/gateway_resolver.rs`

```rust
impl GatewayRefResolver {
    /// Validate that a binding satisfies dependency requirements
    /// Called by linter when checking verb calls
    pub fn validate_dependency_satisfaction(
        &self,
        verb_produces: &VerbProduces,
        verb_args: &[Argument],
        available_bindings: &HashMap<String, BindingInfo>,
    ) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let entity_registry = entity_deps();
        
        // Get what this verb produces
        let prod_type = &verb_produces.produced_type;
        let prod_subtype = verb_produces.resolve_subtype(verb_args);
        
        // Check each required dependency
        for dep in entity_registry.dependencies_of(prod_type, prod_subtype.as_deref()) {
            if dep.kind != DependencyKind::Required {
                continue;
            }
            
            // Look for a binding that satisfies this dependency
            let satisfied = available_bindings.values().any(|binding| {
                binding.ref_type.as_str() == dep.to_type &&
                (dep.to_subtype.is_none() || binding.subtype == dep.to_subtype)
            });
            
            if !satisfied {
                // Check if there's an arg that might provide it
                let has_arg = dep.via_arg.as_ref()
                    .map(|arg| verb_args.iter().any(|a| a.key == *arg))
                    .unwrap_or(false);
                
                if !has_arg {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "Missing required dependency: {} requires {}/{}",
                            format!("{}/{}", prod_type, prod_subtype.as_deref().unwrap_or("*")),
                            dep.to_type,
                            dep.to_subtype.as_deref().unwrap_or("*")
                        ),
                        span: SourceSpan::default(), // Would be set by caller
                        code: DiagnosticCode::MissingDependency,
                        suggestions: vec![],
                    });
                }
            }
        }
        
        diagnostics
    }
}
```

### 5.6 Add DiagnosticCode for Dependencies

File: `rust/src/dsl_v2/validation.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticCode {
    // Existing codes...
    UnknownDocumentType,
    UnknownJurisdiction,
    UnknownRole,
    UnknownEntityType,
    UnknownAttributeId,
    CbuNotFound,
    EntityNotFound,
    DocumentNotFound,
    InvalidValue,
    CircularDependency,
    
    // NEW: Dependency-related codes
    MissingDependency,      // Required dependency not satisfied
    TypeMismatch,           // Entity exists but wrong type
    SubtypeMismatch,        // Entity exists, right type, wrong subtype
    DependencyCycle,        // Circular dependency in entity graph
}
```

---

## Phase 6: Linter / LSP Validation Integration

### 6.1 Unified Entity Reference Validation

File: `rust/src/dsl_v2/lsp_validator.rs`

```rust
use super::entity_deps::{entity_deps, DependencyKind};
use super::ref_resolver::RefResolver;
use super::validation::{TypeConstraint, BindingInfo, Diagnostic, Severity, DiagnosticCode};

/// Validate entity references in a verb call
/// This is the main entry point for linter validation
pub async fn validate_verb_call(
    vc: &VerbCall,
    verb_def: &RuntimeVerb,
    bindings: &BindingContext,
    resolver: &mut impl RefResolver,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entity_registry = entity_deps();
    
    // =========================================================================
    // (A) Validate consumes constraints
    // =========================================================================
    for consume in &verb_def.consumes {
        let Some(arg) = vc.arguments.iter().find(|a| a.key == consume.arg) else {
            if consume.required {
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("Missing required argument: {}", consume.arg),
                    span: vc.span.clone(),
                    code: DiagnosticCode::MissingArgument,
                    suggestions: vec![],
                });
            }
            continue;
        };
        
        // If it's a @symbol reference, validate type and subtype
        if let Some(ref_name) = arg.value.as_symbol() {
            if let Some(binding) = bindings.get(ref_name) {
                // Check type match
                if !type_matches(&binding.ref_type, &consume.consumed_type) {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "Type mismatch: {} expects {} but @{} is {}",
                            consume.arg, consume.consumed_type, ref_name, binding.ref_type
                        ),
                        span: arg.span.clone(),
                        code: DiagnosticCode::TypeMismatch,
                        suggestions: suggest_bindings_of_type(bindings, &consume.consumed_type),
                    });
                    continue;
                }
                
                // Check subtype match if specified
                if let Some(ref expected_subtype) = consume.subtype {
                    if binding.subtype.as_ref() != Some(expected_subtype) {
                        diagnostics.push(Diagnostic {
                            severity: Severity::Error,
                            message: format!(
                                "Subtype mismatch: {} expects {}/{} but @{} is {}/{}",
                                consume.arg, 
                                consume.consumed_type,
                                expected_subtype, 
                                ref_name,
                                binding.ref_type,
                                binding.subtype.as_deref().unwrap_or("(none)")
                            ),
                            span: arg.span.clone(),
                            code: DiagnosticCode::SubtypeMismatch,
                            suggestions: suggest_bindings_of_subtype(bindings, &consume.consumed_type, expected_subtype),
                        });
                    }
                }
            } else {
                // Symbol not found in bindings
                diagnostics.push(Diagnostic {
                    severity: Severity::Error,
                    message: format!("Undefined symbol: @{}", ref_name),
                    span: arg.span.clone(),
                    code: DiagnosticCode::UndefinedSymbol,
                    suggestions: suggest_similar_bindings(bindings, ref_name),
                });
            }
        }
        
        // If it's a literal, resolve via gateway
        if let Some(literal_value) = arg.value.as_string() {
            let constraint = TypeConstraint {
                ref_type: string_to_ref_type(&consume.consumed_type),
                subtype: consume.subtype.clone(),
            };
            
            match resolver.resolve_with_constraint(&constraint, literal_value).await {
                Ok(ResolveResult::NotFound { suggestions }) => {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        message: format!("{} not found: {}", consume.consumed_type, literal_value),
                        span: arg.span.clone(),
                        code: DiagnosticCode::EntityNotFound,
                        suggestions: suggestions.into_iter()
                            .map(|s| s.into_suggestion("Did you mean"))
                            .collect(),
                    });
                }
                Ok(ResolveResult::TypeMismatch { expected_subtype, actual_subtype, .. }) => {
                    diagnostics.push(Diagnostic {
                        severity: Severity::Error,
                        message: format!(
                            "Found {} but expected subtype {} (got {})",
                            literal_value, expected_subtype, actual_subtype.unwrap_or_default()
                        ),
                        span: arg.span.clone(),
                        code: DiagnosticCode::SubtypeMismatch,
                        suggestions: vec![],
                    });
                }
                _ => {} // Found OK
            }
        }
    }
    
    // =========================================================================
    // (B) Validate entity type dependencies are satisfiable
    // =========================================================================
    if let Some(produces) = &verb_def.produces {
        let prod_type = &produces.produced_type;
        let prod_subtype = produces.resolve_subtype(&vc.arguments);
        
        for dep in entity_registry.dependencies_of(prod_type, prod_subtype.as_deref()) {
            if dep.kind != DependencyKind::Required {
                continue;
            }
            
            // Check if there's a binding that can satisfy this dependency
            let satisfied_by_binding = bindings.iter().any(|(_, binding)| {
                type_matches(&binding.ref_type, &dep.to_type) &&
                (dep.to_subtype.is_none() || binding.subtype == dep.to_subtype)
            });
            
            // Check if there's an argument that provides it
            let satisfied_by_arg = dep.via_arg.as_ref()
                .map(|arg| vc.arguments.iter().any(|a| a.key == *arg))
                .unwrap_or(false);
            
            if !satisfied_by_binding && !satisfied_by_arg {
                let dep_desc = format!(
                    "{}/{}",
                    dep.to_type,
                    dep.to_subtype.as_deref().unwrap_or("*")
                );
                
                diagnostics.push(Diagnostic {
                    severity: Severity::Warning, // Warning because it might be provided later
                    message: format!(
                        "Missing dependency: {}/{} requires {} - ensure it's created before this statement",
                        prod_type,
                        prod_subtype.as_deref().unwrap_or("*"),
                        dep_desc
                    ),
                    span: vc.span.clone(),
                    code: DiagnosticCode::MissingDependency,
                    suggestions: suggest_verbs_that_produce(&dep.to_type, dep.to_subtype.as_deref()),
                });
            }
        }
    }
}

/// Suggest bindings that match a given type
fn suggest_bindings_of_type(bindings: &BindingContext, target_type: &str) -> Vec<Suggestion> {
    bindings.iter()
        .filter(|(_, b)| type_matches(&b.ref_type, target_type))
        .map(|(name, _)| Suggestion::new("Use existing binding", format!("@{}", name), 1.0))
        .collect()
}

/// Suggest bindings that match a given type and subtype
fn suggest_bindings_of_subtype(
    bindings: &BindingContext, 
    target_type: &str,
    target_subtype: &str,
) -> Vec<Suggestion> {
    bindings.iter()
        .filter(|(_, b)| {
            type_matches(&b.ref_type, target_type) &&
            b.subtype.as_ref().map(|s| s == target_subtype).unwrap_or(false)
        })
        .map(|(name, _)| Suggestion::new("Use existing binding", format!("@{}", name), 1.0))
        .collect()
}

/// Suggest verbs that produce a given type
fn suggest_verbs_that_produce(target_type: &str, target_subtype: Option<&str>) -> Vec<Suggestion> {
    let registry = runtime_registry();
    
    registry.iter()
        .filter(|(_, verb)| {
            verb.produces.as_ref().map(|p| {
                p.produced_type == target_type &&
                (target_subtype.is_none() || p.subtype.as_deref() == target_subtype)
            }).unwrap_or(false)
        })
        .map(|(key, _)| Suggestion::new(
            &format!("Create with {}", key),
            format!("({} ...)", key),
            0.8
        ))
        .collect()
}
```

### 6.2 Auto-Complete Suggestions

File: `rust/src/dsl_v2/suggestions.rs`

```rust
use super::entity_deps::entity_deps;
use super::runtime_registry::runtime_registry;

/// Suggest valid subtypes for a given entity type
pub fn suggest_subtypes_for_type(base_type: &str) -> Vec<CompletionItem> {
    let registry = entity_deps();
    
    registry.known_types()
        .filter(|(t, st)| *t == base_type && st.is_some())
        .map(|(_, st)| {
            let subtype = st.unwrap();
            CompletionItem {
                label: subtype.to_string(),
                kind: CompletionItemKind::EnumMember,
                detail: Some(format!("{} subtype", base_type)),
                documentation: get_subtype_documentation(base_type, subtype),
                ..Default::default()
            }
        })
        .collect()
}

/// Suggest resource types for service-resource.provision
pub fn suggest_resource_types() -> Vec<CompletionItem> {
    suggest_subtypes_for_type("resource_instance")
}

/// Suggest entity subtypes (fund types, entity types)
pub fn suggest_entity_subtypes() -> Vec<CompletionItem> {
    suggest_subtypes_for_type("entity")
}

/// Suggest verbs that can create a dependency for another verb
pub fn suggest_dependency_creators(
    for_type: &str,
    for_subtype: Option<&str>,
) -> Vec<CompletionItem> {
    let entity_registry = entity_deps();
    let verb_registry = runtime_registry();
    
    // Find what this type depends on
    let deps = entity_registry.dependencies_of(for_type, for_subtype);
    
    // For each dependency, find verbs that produce it
    deps.iter()
        .flat_map(|dep| {
            verb_registry.iter()
                .filter(|(_, verb)| {
                    verb.produces.as_ref().map(|p| {
                        p.produced_type == dep.to_type &&
                        (dep.to_subtype.is_none() || p.subtype == dep.to_subtype)
                    }).unwrap_or(false)
                })
                .map(|(key, verb)| CompletionItem {
                    label: key.clone(),
                    kind: CompletionItemKind::Function,
                    detail: Some(format!("Creates {}/{}", dep.to_type, dep.to_subtype.as_deref().unwrap_or("*"))),
                    documentation: Some(verb.description.clone()),
                    insert_text: Some(format!("({} )", key)),
                    ..Default::default()
                })
        })
        .collect()
}

/// Get documentation for a subtype from entity_type_dependencies
fn get_subtype_documentation(base_type: &str, subtype: &str) -> Option<String> {
    let registry = entity_deps();
    let deps = registry.dependencies_of(base_type, Some(subtype));
    
    if deps.is_empty() {
        return None;
    }
    
    let dep_list: Vec<String> = deps.iter()
        .map(|d| format!("  → {}/{}", d.to_type, d.to_subtype.as_deref().unwrap_or("*")))
        .collect();
    
    Some(format!("Dependencies:\n{}", dep_list.join("\n")))
}
```

### 6.3 LSP Hover Information

File: `rust/src/dsl_v2/lsp_validator.rs` (add to existing)

```rust
/// Get hover info for an entity reference
pub fn hover_info_for_binding(binding: &BindingInfo) -> HoverInfo {
    let entity_registry = entity_deps();
    
    let mut lines = vec![
        format!("**@{}**", binding.name),
        format!("Type: `{}`", binding.ref_type),
    ];
    
    if let Some(ref subtype) = binding.subtype {
        lines.push(format!("Subtype: `{}`", subtype));
        
        // Show dependencies
        let deps = entity_registry.dependencies_of(&binding.ref_type, Some(subtype));
        if !deps.is_empty() {
            lines.push("".to_string());
            lines.push("**Requires:**".to_string());
            for dep in deps {
                let dep_desc = format!(
                    "- `{}/{}`{}",
                    dep.to_type,
                    dep.to_subtype.as_deref().unwrap_or("*"),
                    if dep.kind == DependencyKind::Optional { " (optional)" } else { "" }
                );
                lines.push(dep_desc);
            }
        }
        
        // Show what depends on this
        let dependents = entity_registry.dependents_of(&binding.ref_type, Some(subtype));
        if !dependents.is_empty() {
            lines.push("".to_string());
            lines.push("**Required by:**".to_string());
            for dep in dependents {
                lines.push(format!("- `{}/{}`", dep.from_type, dep.from_subtype.as_deref().unwrap_or("*")));
            }
        }
    }
    
    HoverInfo {
        contents: lines.join("\n"),
        range: binding.defined_at.clone(),
    }
}
```

### 6.4 Add Reverse Lookup to EntityDependencyRegistry

File: `rust/src/dsl_v2/entity_deps.rs` (add method)

```rust
impl EntityDependencyRegistry {
    /// Get entities that depend on a given type/subtype (reverse lookup)
    /// Used for hover info and impact analysis
    pub fn dependents_of(&self, to_type: &str, to_subtype: Option<&str>) -> Vec<DependentInfo> {
        self.deps.iter()
            .filter_map(|((from_type, from_subtype), deps)| {
                let matching_deps: Vec<_> = deps.iter()
                    .filter(|d| {
                        d.to_type == to_type &&
                        (to_subtype.is_none() || d.to_subtype.as_deref() == to_subtype)
                    })
                    .collect();
                
                if matching_deps.is_empty() {
                    None
                } else {
                    Some(DependentInfo {
                        from_type: from_type.clone(),
                        from_subtype: from_subtype.clone(),
                    })
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct DependentInfo {
    pub from_type: String,
    pub from_subtype: Option<String>,
}
```

---

## Phase 7: Cleanup

### 7.1 Remove Redundant Code

After the unified model is working:

1. **Delete** `onboarding.rs::ResourceDependencyGraph` struct
2. **Delete** `onboarding.rs::compute_stages()` function  
3. **Delete** `onboarding.rs::build_resource_dependency_graph()` function
4. **Delete** `execution_plan.rs::topological_sort()` private function
5. **Simplify** `onboarding.rs::generate_provisioning_dsl()` - no more `:depends-on` generation

### 7.2 Simplified Onboarding DSL Generation

```rust
/// Generate DSL for onboarding - compiler handles ordering
fn generate_provisioning_dsl(
    cbu_id: &Uuid,
    resources: &[ResourceToProvision],
) -> String {
    // Just emit provisions in any order - compiler sorts
    resources.iter()
        .map(|r| format!(
            "(service-resource.provision :cbu-id \"{}\" :resource-type \"{}\" :as @res_{})",
            cbu_id, 
            r.resource_code, 
            r.resource_code.to_lowercase()
        ))
        .collect::<Vec<_>>()
        .join("\n")
}
```

---

## Phase 8: Testing

### 8.1 Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    fn test_registry() -> EntityDependencyRegistry {
        let mut deps = HashMap::new();
        
        // fund_sub -> fund_umbrella
        deps.insert(
            ("entity".into(), Some("fund_sub".into())),
            vec![EntityDep {
                to_type: "entity".into(),
                to_subtype: Some("fund_umbrella".into()),
                via_arg: Some("umbrella-id".into()),
                kind: DependencyKind::Required,
                priority: 100,
            }]
        );
        
        // CUSTODY_ACCT -> SETTLE_ACCT
        deps.insert(
            ("resource_instance".into(), Some("CUSTODY_ACCT".into())),
            vec![EntityDep {
                to_type: "resource_instance".into(),
                to_subtype: Some("SETTLE_ACCT".into()),
                via_arg: None,
                kind: DependencyKind::Required,
                priority: 100,
            }]
        );
        
        EntityDependencyRegistry { deps }
    }
    
    #[test]
    fn test_entity_deps_lookup() {
        let registry = test_registry();
        
        let deps = registry.dependencies_of("entity", Some("fund_sub"));
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].to_subtype, Some("fund_umbrella".into()));
    }
    
    #[test]
    fn test_resource_deps_lookup() {
        let registry = test_registry();
        
        let deps = registry.dependencies_of("resource_instance", Some("CUSTODY_ACCT"));
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].to_subtype, Some("SETTLE_ACCT".into()));
    }
    
    #[test]
    fn test_topo_sort_entity_deps() {
        let registry = test_registry();
        
        // DSL with out-of-order statements
        let dsl = r#"
            (fund.create-sub-fund :umbrella-id @umbrella :name "Sub 1" :as @sub)
            (fund.create-umbrella :legal-entity-id @entity :name "Umbrella" :as @umbrella)
        "#;
        
        let program = parse_program(dsl).unwrap();
        let result = topological_sort_unified(
            &program,
            &BindingContext::new(),
            &registry,
            &runtime_registry(),
        ).unwrap();
        
        // umbrella should come before sub-fund
        assert!(result.reordered);
        let sorted_verbs: Vec<_> = result.program.statements.iter()
            .filter_map(|s| match s {
                Statement::VerbCall(vc) => Some(&vc.verb),
                _ => None,
            })
            .collect();
        
        assert_eq!(sorted_verbs, vec!["create-umbrella", "create-sub-fund"]);
    }
}
```

### 8.2 Integration Test

```rust
#[tokio::test]
async fn test_full_fund_hierarchy_ordering() {
    let pool = test_pool().await;
    
    // DSL with intentionally wrong order
    let dsl = r#"
        (fund.create-share-class :sub-fund-id @sub :name "Class A" :as @class)
        (fund.create-sub-fund :umbrella-id @umbrella :name "Sub 1" :as @sub)
        (entity.create-limited-company :name "FundCo" :as @entity)
        (fund.create-umbrella :legal-entity-id @entity :name "Umbrella" :as @umbrella)
    "#;
    
    let executor = DslExecutor::new(pool.clone());
    let mut ctx = ExecutionContext::new();
    
    // Should succeed - compiler reorders
    let result = executor.execute_dsl(dsl, &mut ctx).await;
    assert!(result.is_ok());
    
    // Verify all entities created
    assert!(ctx.get("entity").is_some());
    assert!(ctx.get("umbrella").is_some());
    assert!(ctx.get("sub").is_some());
    assert!(ctx.get("class").is_some());
}
```

---

## Implementation Order

1. **Phase 1.1** - Create `entity_type_dependencies` table
2. **Phase 1.2** - Seed dependencies (structural, fund hierarchy, resources)
3. **Phase 2.1** - Extend `VerbProduces` struct with `subtype_from_arg`
4. **Phase 2.2** - Update verb YAML files (`service-resource.yaml`, `fund.yaml`)
5. **Phase 3.1** - Create `entity_deps.rs` module
6. **Phase 3.2** - Register in `mod.rs`
7. **Phase 4.1** - Implement `topological_sort_unified` in `topo_sort.rs`
8. **Phase 4.2** - Wire into `execution_plan.rs`
9. **Phase 5.1** - Extend `RefType` and `BindingInfo` with subtype
10. **Phase 5.2** - Extend `LookupConfig` with `entity_subtype`
11. **Phase 5.3** - Update verb YAML with subtype constraints in lookup
12. **Phase 5.4** - Integrate into `GatewayRefResolver`
13. **Phase 6.1** - Implement unified `validate_verb_call` in linter
14. **Phase 6.2** - Auto-complete suggestions integration
15. **Phase 6.3** - LSP hover information
16. **Phase 8** - Run tests (unit + integration)
17. **Phase 7** - Cleanup redundant code

---

## Files to Create/Modify

| File | Action | Description |
|------|--------|-------------|
| SQL migration | Create | `entity_type_dependencies` table |
| `rust/src/dsl_v2/config/types.rs` | Modify | Add `subtype_from_arg` to `VerbProduces`, `entity_subtype` to `LookupConfig` |
| `rust/src/dsl_v2/validation.rs` | Modify | Extend `RefType`, add `TypeConstraint`, extend `BindingInfo` |
| `rust/src/dsl_v2/entity_deps.rs` | Create | Entity dependency registry with `dependents_of` |
| `rust/src/dsl_v2/mod.rs` | Modify | Export `entity_deps` module |
| `rust/src/dsl_v2/topo_sort.rs` | Modify | Add `topological_sort_unified` |
| `rust/src/dsl_v2/execution_plan.rs` | Modify | Use unified topo sort |
| `rust/src/dsl_v2/ref_resolver.rs` | Modify | Add `TypeConstraint` support |
| `rust/src/dsl_v2/gateway_resolver.rs` | Modify | Add `resolve_with_constraint`, dependency validation |
| `rust/src/dsl_v2/lsp_validator.rs` | Modify | Add unified `validate_verb_call`, hover info |
| `rust/src/dsl_v2/suggestions.rs` | Modify | Add subtype and dependency creator suggestions |
| `rust/config/verbs/service-resource.yaml` | Modify | Add `subtype_from_arg` to produces |
| `rust/config/verbs/fund.yaml` | Modify | Add `subtype` to produces/consumes, `entity_subtype` to lookups |
| `rust/src/dsl_v2/custom_ops/onboarding.rs` | Modify | Remove redundant graph code |

---

## Verification Queries

```sql
-- View all dependencies for a type
SELECT from_type, from_subtype, to_type, to_subtype, via_arg, dependency_kind
FROM "ob-poc".entity_type_dependencies
WHERE from_type = 'resource_instance'
ORDER BY from_subtype, priority;

-- Find root types (no dependencies)
SELECT DISTINCT from_type, from_subtype
FROM "ob-poc".entity_type_dependencies
WHERE (from_type, from_subtype) NOT IN (
    SELECT to_type, to_subtype 
    FROM "ob-poc".entity_type_dependencies
    WHERE to_subtype IS NOT NULL
);

-- Detect potential cycles
WITH RECURSIVE dep_chain AS (
    SELECT from_type, from_subtype, to_type, to_subtype, 
           ARRAY[from_type || '/' || COALESCE(from_subtype, '*')] as path
    FROM "ob-poc".entity_type_dependencies
    
    UNION ALL
    
    SELECT d.from_type, d.from_subtype, e.to_type, e.to_subtype,
           d.path || (e.from_type || '/' || COALESCE(e.from_subtype, '*'))
    FROM dep_chain d
    JOIN "ob-poc".entity_type_dependencies e 
        ON d.to_type = e.from_type 
        AND (d.to_subtype = e.from_subtype OR e.from_subtype IS NULL)
    WHERE NOT (e.from_type || '/' || COALESCE(e.from_subtype, '*')) = ANY(d.path)
)
SELECT * FROM dep_chain 
WHERE to_type || '/' || COALESCE(to_subtype, '*') = ANY(path);
```