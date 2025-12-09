# TODO — Lifecycle-Aware DSL Execution Planner

> **Goal:**  
> Extend the existing `execution_plan.rs` compiler to support **entity lifecycle awareness**, **state-based ordering**, and **synthetic step injection** — all driven by **config**, not hardcoded Rust.

This document is the implementation specification for Claude-Code.  
It assumes no prior chat history — everything needed is captured here.

---

## 0. Context & Current State

### 0.1 Domain

- Platform: **ob-poc** — KYC / onboarding / UBO / services for institutional / fund clients.
- Core unifying synthetic entity: **CBU** (Client Business Unit).
- Two big functional domains:
  - **KYC / UBO**: KYC cases, workstreams, UBO graph, docs, screening, red flags.
  - **Onboarding / Services**: CBU, products, services, service_resources, resource instances.

### 0.2 Existing DSL Pipeline

```
Source → Parser → AST → Enrichment → CSG Lint → Semantic Validate → Compile → Execute
                          ↓                            ↓
                     YAML verb defs              EntityGateway
```

**Key files:**
- `rust/src/dsl_v2/parser.rs` — NOM-based S-expression parser
- `rust/src/dsl_v2/enrichment.rs` — Converts raw strings to EntityRefs using verb YAML
- `rust/src/dsl_v2/csg_linter.rs` — Context-sensitive grammar rules
- `rust/src/dsl_v2/semantic_validator.rs` — Validates refs against DB
- `rust/src/dsl_v2/execution_plan.rs` — **Compiles AST → ExecutionPlan** (this is where we extend)
- `rust/src/dsl_v2/topo_sort.rs` — Topological sort for `@binding` dependencies
- `rust/src/dsl_v2/executor.rs` — Executes plan against DB

### 0.3 What Already Exists

| Component | Location | Current Capability |
|-----------|----------|-------------------|
| Topological sort | `topo_sort.rs` | Sorts by `@binding` producer/consumer |
| Execution plan compiler | `execution_plan.rs` | Flattens nested verbs, injects parent FKs |
| `PARENT_FK_MAP` | `execution_plan.rs` | Hardcoded FK relationships (20+ entries) |
| `produces` / `consumes` | Verb YAML | Declares what verbs produce/consume |
| Verb registry | `verb_registry.rs` | Unified access to all verb definitions |

### 0.4 What's Missing (This TODO Addresses)

1. **Entity state tracking** — No concept of "CBU is in DRAFT state"
2. **Lifecycle transitions** — No validation that `mark-validated` requires state DRAFT
3. **Canonical creator injection** — No auto-synthesis of missing `create` steps
4. **Config-driven FK map** — `PARENT_FK_MAP` is hardcoded, should be in config
5. **Entity resolution by natural key** — Semantic validator resolves by PK only

---

## 1. Design Principles

### 1.1 Two Config Sources Only

All planner semantics come from exactly two places:

1. **`config/ontology/entity_taxonomy.yaml`** — Entity definitions, lifecycles, FK relationships
2. **Verb YAML files** (`config/verbs/*.yaml`) — Extended with lifecycle pre/post conditions

No additional config files. No `verb_semantics.yaml`. No `dependencies.yaml`. No `entity_resolver.yaml`.

### 1.2 Extend, Don't Duplicate

- Extend `execution_plan.rs` — don't create parallel `planner.rs`
- Extend `topo_sort.rs` — add lifecycle edges to existing sort
- Extend verb YAML schema — add `lifecycle` section to verb definitions
- Move `PARENT_FK_MAP` to config — eliminate hardcoding

### 1.3 Warnings First, Errors Later

- Phase 1: Lifecycle violations emit **warnings** (don't break existing DSL)
- Phase 2: Opt-in **strict mode** makes violations errors
- DB enforcement functions are optional future work

---

## 2. Config Schema Definitions

### 2.1 Entity Taxonomy Config

**File:** `rust/config/ontology/entity_taxonomy.yaml`

```yaml
# Entity Taxonomy Configuration
# Defines entity types, their DB mappings, lifecycles, and relationships

version: "1.0"

entities:
  # ============================================================================
  # CBU - Client Business Unit (core synthetic entity)
  # ============================================================================
  cbu:
    description: "Client Business Unit - the core onboarding subject"
    category: subject
    
    db:
      schema: ob-poc
      table: cbus
      pk: cbu_id
      
    search_keys:
      - column: name
        unique: false
      - columns: [name, jurisdiction]
        unique: true
        
    lifecycle:
      status_column: status
      states:
        - DISCOVERED
        - DRAFT
        - PENDING_VALIDATION
        - VALIDATED
        - ACTIVE
        - SUSPENDED
        - TERMINATED
      transitions:
        - from: DISCOVERED
          to: [DRAFT]
        - from: DRAFT
          to: [PENDING_VALIDATION, TERMINATED]
        - from: PENDING_VALIDATION
          to: [VALIDATED, DRAFT]
        - from: VALIDATED
          to: [ACTIVE, SUSPENDED]
        - from: ACTIVE
          to: [SUSPENDED, TERMINATED]
        - from: SUSPENDED
          to: [ACTIVE, TERMINATED]
      initial_state: DISCOVERED
      
    implicit_create:
      allowed: true
      canonical_verb: cbu.create
      # Minimum args that must be inferrable for auto-create
      required_args: [name, jurisdiction]

  # ============================================================================
  # Entity - Legal/natural persons
  # ============================================================================
  entity:
    description: "Legal or natural person entity"
    category: entity
    
    db:
      schema: ob-poc
      table: entities
      pk: entity_id
      type_column: type_id
      type_lookup_table: entity_types
      
    search_keys:
      - column: name
        unique: false
      - column: registration_number
        unique: true
        applies_to: [limited_company, partnership, trust]
        
    subtypes:
      source_table: entity_types
      code_column: type_code
      # Subtypes get their own extension tables
      extensions:
        proper_person: entity_proper_persons
        limited_company: entity_limited_companies
        trust: entity_trusts
        partnership: entity_partnerships
        
    lifecycle:
      status_column: status
      states: [DRAFT, ACTIVE, INACTIVE]
      transitions:
        - from: DRAFT
          to: [ACTIVE]
        - from: ACTIVE
          to: [INACTIVE]
      initial_state: DRAFT
      
    implicit_create:
      allowed: true
      # Dynamic: entity.create-{subtype}
      canonical_verb_pattern: "entity.create-{subtype}"
      required_args: [name]

  # ============================================================================
  # KYC Case
  # ============================================================================
  kyc_case:
    description: "KYC investigation case"
    category: kyc
    
    db:
      schema: kyc
      table: cases
      pk: case_id
      
    search_keys:
      - column: case_reference
        unique: true
        
    lifecycle:
      status_column: status
      states:
        - OPEN
        - IN_PROGRESS
        - PENDING_DECISION
        - APPROVED
        - REJECTED
        - CLOSED
      transitions:
        - from: OPEN
          to: [IN_PROGRESS, CLOSED]
        - from: IN_PROGRESS
          to: [PENDING_DECISION, CLOSED]
        - from: PENDING_DECISION
          to: [APPROVED, REJECTED, IN_PROGRESS]
        - from: [APPROVED, REJECTED]
          to: [CLOSED]
      initial_state: OPEN
      
    implicit_create:
      allowed: true
      canonical_verb: kyc.create-case
      required_args: [cbu-id, case-type]

  # ============================================================================
  # KYC Workstream
  # ============================================================================
  kyc_workstream:
    description: "Entity-level KYC workstream within a case"
    category: kyc
    
    db:
      schema: kyc
      table: entity_workstreams
      pk: workstream_id
      
    search_keys:
      - columns: [case_id, entity_id]
        unique: true
        
    lifecycle:
      status_column: status
      states:
        - PENDING
        - IN_PROGRESS
        - COMPLETE
        - BLOCKED
      transitions:
        - from: PENDING
          to: [IN_PROGRESS]
        - from: IN_PROGRESS
          to: [COMPLETE, BLOCKED]
        - from: BLOCKED
          to: [IN_PROGRESS]
      initial_state: PENDING
      
    implicit_create:
      allowed: true
      canonical_verb: kyc.create-workstream
      required_args: [case-id, entity-id]

  # ============================================================================
  # Document
  # ============================================================================
  document:
    description: "Document in the document catalog"
    category: document
    
    db:
      schema: ob-poc
      table: document_catalog
      pk: doc_id
      
    search_keys:
      - column: doc_reference
        unique: true
      - columns: [entity_id, document_type_id]
        unique: false
        
    lifecycle:
      status_column: status
      states:
        - PENDING
        - RECEIVED
        - VERIFIED
        - REJECTED
        - EXPIRED
      transitions:
        - from: PENDING
          to: [RECEIVED, REJECTED]
        - from: RECEIVED
          to: [VERIFIED, REJECTED]
        - from: VERIFIED
          to: [EXPIRED]
      initial_state: PENDING
      
    implicit_create:
      allowed: true
      canonical_verb: document.create
      required_args: [entity-id, document-type]

  # ============================================================================
  # UBO Record
  # ============================================================================
  ubo_record:
    description: "Ultimate Beneficial Owner registry entry"
    category: ubo
    
    db:
      schema: ob-poc
      table: ubo_registry
      pk: ubo_id
      
    search_keys:
      - columns: [subject_entity_id, person_entity_id]
        unique: true
        
    lifecycle:
      status_column: verification_status
      states:
        - DECLARED
        - EVIDENCED
        - VERIFIED
        - DISPUTED
      transitions:
        - from: DECLARED
          to: [EVIDENCED, DISPUTED]
        - from: EVIDENCED
          to: [VERIFIED, DISPUTED]
        - from: DISPUTED
          to: [EVIDENCED, DECLARED]
      initial_state: DECLARED
      
    implicit_create:
      allowed: true
      canonical_verb: ubo.declare
      required_args: [subject-entity-id, person-entity-id, ownership-percentage]

  # ============================================================================
  # Resource Instance
  # ============================================================================
  cbu_resource_instance:
    description: "Provisioned resource instance for a CBU"
    category: resource
    
    db:
      schema: ob-poc
      table: cbu_resource_instances
      pk: instance_id
      
    search_keys:
      - column: instance_identifier
        unique: true
      - columns: [cbu_id, resource_type_id]
        unique: false
        
    lifecycle:
      status_column: status
      states:
        - PENDING
        - PROVISIONED
        - ACTIVE
        - SUSPENDED
        - DECOMMISSIONED
      transitions:
        - from: PENDING
          to: [PROVISIONED]
        - from: PROVISIONED
          to: [ACTIVE]
        - from: ACTIVE
          to: [SUSPENDED, DECOMMISSIONED]
        - from: SUSPENDED
          to: [ACTIVE, DECOMMISSIONED]
      initial_state: PENDING
      
    implicit_create:
      allowed: true
      canonical_verb: resource.allocate
      required_args: [cbu-id, resource-type]

# ============================================================================
# FK Relationships (replaces PARENT_FK_MAP)
# ============================================================================
relationships:
  # Pattern: when child verb runs under parent context, inject this FK
  # (parent_domain, child_domain) -> child_arg_name
  
  # Same-domain self-references
  - parent: cbu
    child: cbu
    fk_arg: cbu-id
    
  - parent: entity
    child: entity
    fk_arg: entity-id
    
  - parent: document
    child: document
    fk_arg: document-id

  # CBU as parent
  - parent: cbu
    child: document
    fk_arg: cbu-id
    
  - parent: cbu
    child: kyc
    fk_arg: cbu-id
    
  - parent: cbu
    child: screening
    fk_arg: cbu-id
    
  - parent: cbu
    child: risk
    fk_arg: cbu-id
    
  - parent: cbu
    child: monitoring
    fk_arg: cbu-id
    
  - parent: cbu
    child: resource
    fk_arg: cbu-id

  # Entity as parent
  - parent: entity
    child: document
    fk_arg: entity-id
    
  - parent: entity
    child: screening
    fk_arg: entity-id
    
  - parent: entity
    child: ubo
    fk_arg: entity-id

  # KYC relationships
  - parent: kyc
    child: screening
    fk_arg: case-id
    
  - parent: kyc
    child: decision
    fk_arg: case-id

  # Product/Service relationships
  - parent: product
    child: service
    fk_arg: product-id
    
  - parent: service
    child: resource
    fk_arg: service-id
```

### 2.2 Verb YAML Lifecycle Extension

**Extend existing verb YAML files** with a `lifecycle` section:

```yaml
# Example: rust/config/verbs/cbu.yaml (additions)
domains:
  cbu:
    verbs:
      create:
        # ... existing config ...
        produces:
          type: cbu
          initial_state: DISCOVERED  # NEW: what state the created entity starts in
          
      ensure:
        # ... existing config ...
        produces:
          type: cbu
          initial_state: DISCOVERED
          resolved: false  # May match existing record
          
      mark-validated:
        description: "Transition CBU to VALIDATED state"
        behavior: crud
        crud:
          operation: update
          table: cbus
          schema: ob-poc
          key: cbu_id
          set_values:
            status: VALIDATED
        consumes:
          - arg: cbu-id
            type: cbu
            required: true
        # NEW: Lifecycle section
        lifecycle:
          entity_arg: cbu-id
          requires_states:
            - DRAFT
            - PENDING_VALIDATION
          transitions_to: VALIDATED
          precondition_checks:
            - check_cbu_evidence_completeness
            - check_cbu_invariants
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
            lookup:
              table: cbus
              entity_type: cbu
              schema: ob-poc
              search_key: name
              primary_key: cbu_id
        returns:
          type: affected
          
      set-status:
        description: "Set CBU status (generic transition)"
        behavior: crud
        crud:
          operation: update
          table: cbus
          schema: ob-poc
          key: cbu_id
        consumes:
          - arg: cbu-id
            type: cbu
            required: true
        lifecycle:
          entity_arg: cbu-id
          # No requires_states = any state allowed
          transitions_to_arg: status  # NEW: transition target comes from arg value
        args:
          - name: cbu-id
            type: uuid
            required: true
            maps_to: cbu_id
          - name: status
            type: string
            required: true
            maps_to: status
            validation:
              enum:
                - DRAFT
                - PENDING_VALIDATION
                - VALIDATED
                - ACTIVE
                - SUSPENDED
                - TERMINATED
        returns:
          type: affected
```

---

## 3. Rust Type Definitions

### 3.1 New Types in `config/types.rs`

```rust
// Add to rust/src/dsl_v2/config/types.rs

// =============================================================================
// ENTITY TAXONOMY TYPES
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityTaxonomyConfig {
    pub version: String,
    #[serde(default)]
    pub entities: HashMap<String, EntityDef>,
    #[serde(default)]
    pub relationships: Vec<FkRelationship>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityDef {
    pub description: String,
    #[serde(default)]
    pub category: Option<String>,
    pub db: EntityDbConfig,
    #[serde(default)]
    pub search_keys: Vec<SearchKeyDef>,
    #[serde(default)]
    pub lifecycle: Option<EntityLifecycle>,
    #[serde(default)]
    pub subtypes: Option<SubtypeConfig>,
    #[serde(default)]
    pub implicit_create: Option<ImplicitCreateConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityDbConfig {
    pub schema: String,
    pub table: String,
    pub pk: String,
    #[serde(default)]
    pub type_column: Option<String>,
    #[serde(default)]
    pub type_lookup_table: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SearchKeyDef {
    Single {
        column: String,
        #[serde(default)]
        unique: bool,
        #[serde(default)]
        applies_to: Option<Vec<String>>,
    },
    Composite {
        columns: Vec<String>,
        #[serde(default)]
        unique: bool,
        #[serde(default)]
        applies_to: Option<Vec<String>>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityLifecycle {
    pub status_column: String,
    pub states: Vec<String>,
    #[serde(default)]
    pub transitions: Vec<StateTransition>,
    #[serde(default)]
    pub initial_state: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StateTransition {
    pub from: StateRef,
    pub to: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StateRef {
    Single(String),
    Multiple(Vec<String>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SubtypeConfig {
    pub source_table: String,
    pub code_column: String,
    #[serde(default)]
    pub extensions: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImplicitCreateConfig {
    #[serde(default)]
    pub allowed: bool,
    #[serde(default)]
    pub canonical_verb: Option<String>,
    #[serde(default)]
    pub canonical_verb_pattern: Option<String>,
    #[serde(default)]
    pub required_args: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FkRelationship {
    pub parent: String,
    pub child: String,
    pub fk_arg: String,
}

// =============================================================================
// VERB LIFECYCLE EXTENSION
// =============================================================================

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbLifecycle {
    /// Which argument carries the target entity
    pub entity_arg: String,
    /// States the entity must be in for this verb to execute
    #[serde(default)]
    pub requires_states: Vec<String>,
    /// State the entity transitions to after execution
    #[serde(default)]
    pub transitions_to: Option<String>,
    /// Arg name that contains the target state (for generic set-state verbs)
    #[serde(default)]
    pub transitions_to_arg: Option<String>,
    /// Named precondition checks to run
    #[serde(default)]
    pub precondition_checks: Vec<String>,
}
```

### 3.2 Extend VerbConfig

```rust
// In rust/src/dsl_v2/config/types.rs, extend VerbConfig:

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbConfig {
    pub description: String,
    pub behavior: VerbBehavior,
    #[serde(default)]
    pub crud: Option<CrudConfig>,
    #[serde(default)]
    pub handler: Option<String>,
    #[serde(default)]
    pub args: Vec<ArgConfig>,
    #[serde(default)]
    pub returns: Option<ReturnsConfig>,
    #[serde(default)]
    pub produces: Option<VerbProduces>,
    #[serde(default)]
    pub consumes: Vec<VerbConsumes>,
    // NEW: Lifecycle semantics
    #[serde(default)]
    pub lifecycle: Option<VerbLifecycle>,
}
```

### 3.3 Extend VerbProduces

```rust
// In rust/src/dsl_v2/config/types.rs, extend VerbProduces:

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VerbProduces {
    #[serde(rename = "type")]
    pub produced_type: String,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub resolved: bool,
    // NEW: Initial state for created entities
    #[serde(default)]
    pub initial_state: Option<String>,
}
```

---

## 4. OntologyService Implementation

### 4.1 Module Structure

```
rust/src/ontology/
├── mod.rs           # Module exports
├── taxonomy.rs      # EntityTaxonomy struct + loading
├── lifecycle.rs     # Lifecycle validation helpers
└── service.rs       # OntologyService (unified access)
```

### 4.2 `mod.rs`

```rust
//! Ontology module - Entity taxonomy and lifecycle definitions
//!
//! Provides config-driven entity metadata for the DSL planner.

mod lifecycle;
mod service;
mod taxonomy;

pub use lifecycle::*;
pub use service::OntologyService;
pub use taxonomy::*;
```

### 4.3 `taxonomy.rs`

```rust
//! Entity taxonomy loading and access

use crate::dsl_v2::config::types::{EntityDef, EntityTaxonomyConfig, FkRelationship};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;

/// Loaded entity taxonomy
#[derive(Debug, Clone)]
pub struct EntityTaxonomy {
    entities: HashMap<String, EntityDef>,
    fk_relationships: Vec<FkRelationship>,
    /// Index: (parent_domain, child_domain) -> fk_arg
    fk_index: HashMap<(String, String), String>,
}

impl EntityTaxonomy {
    /// Load from YAML file
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .with_context(|| format!("Failed to read {}", path.as_ref().display()))?;
        let config: EntityTaxonomyConfig = serde_yaml::from_str(&content)
            .with_context(|| "Failed to parse entity taxonomy YAML")?;
        
        // Build FK index
        let mut fk_index = HashMap::new();
        for rel in &config.relationships {
            fk_index.insert(
                (rel.parent.clone(), rel.child.clone()),
                rel.fk_arg.clone(),
            );
        }
        
        Ok(Self {
            entities: config.entities,
            fk_relationships: config.relationships,
            fk_index,
        })
    }
    
    /// Get entity definition by type code
    pub fn get(&self, entity_type: &str) -> Option<&EntityDef> {
        self.entities.get(entity_type)
    }
    
    /// Get FK arg for parent->child relationship
    pub fn get_fk(&self, parent_domain: &str, child_domain: &str) -> Option<&str> {
        self.fk_index
            .get(&(parent_domain.to_string(), child_domain.to_string()))
            .map(|s| s.as_str())
    }
    
    /// Get all entity types
    pub fn entity_types(&self) -> impl Iterator<Item = &str> {
        self.entities.keys().map(|s| s.as_str())
    }
    
    /// Check if entity type supports implicit create
    pub fn allows_implicit_create(&self, entity_type: &str) -> bool {
        self.entities
            .get(entity_type)
            .and_then(|e| e.implicit_create.as_ref())
            .map(|ic| ic.allowed)
            .unwrap_or(false)
    }
    
    /// Get canonical creator verb for entity type
    pub fn canonical_creator(&self, entity_type: &str, subtype: Option<&str>) -> Option<String> {
        let entity = self.entities.get(entity_type)?;
        let ic = entity.implicit_create.as_ref()?;
        
        if let Some(ref pattern) = ic.canonical_verb_pattern {
            // Dynamic pattern like "entity.create-{subtype}"
            subtype.map(|st| pattern.replace("{subtype}", st))
        } else {
            ic.canonical_verb.clone()
        }
    }
}
```

### 4.4 `lifecycle.rs`

```rust
//! Lifecycle validation helpers

use crate::dsl_v2::config::types::{EntityLifecycle, StateRef};

/// Check if a state transition is valid
pub fn is_valid_transition(lifecycle: &EntityLifecycle, from: &str, to: &str) -> bool {
    for transition in &lifecycle.transitions {
        let from_matches = match &transition.from {
            StateRef::Single(s) => s == from,
            StateRef::Multiple(states) => states.iter().any(|s| s == from),
        };
        
        if from_matches && transition.to.contains(&to.to_string()) {
            return true;
        }
    }
    false
}

/// Get valid next states from current state
pub fn valid_next_states(lifecycle: &EntityLifecycle, current: &str) -> Vec<&str> {
    let mut next = Vec::new();
    for transition in &lifecycle.transitions {
        let from_matches = match &transition.from {
            StateRef::Single(s) => s == current,
            StateRef::Multiple(states) => states.iter().any(|s| s == current),
        };
        
        if from_matches {
            next.extend(transition.to.iter().map(|s| s.as_str()));
        }
    }
    next
}

/// Check if a state is valid for this lifecycle
pub fn is_valid_state(lifecycle: &EntityLifecycle, state: &str) -> bool {
    lifecycle.states.contains(&state.to_string())
}
```

### 4.5 `service.rs`

```rust
//! OntologyService - unified access to entity metadata

use super::taxonomy::EntityTaxonomy;
use crate::dsl_v2::config::types::{EntityDef, EntityLifecycle, VerbLifecycle};
use crate::dsl_v2::runtime_registry::{runtime_registry, RuntimeVerb};
use anyhow::Result;
use std::path::Path;
use std::sync::OnceLock;

static ONTOLOGY: OnceLock<OntologyService> = OnceLock::new();

/// Unified ontology service
pub struct OntologyService {
    taxonomy: EntityTaxonomy,
}

impl OntologyService {
    /// Load ontology from config directory
    pub fn load(config_dir: impl AsRef<Path>) -> Result<Self> {
        let taxonomy_path = config_dir.as_ref().join("ontology/entity_taxonomy.yaml");
        let taxonomy = EntityTaxonomy::load(&taxonomy_path)?;
        Ok(Self { taxonomy })
    }
    
    /// Get global instance (loads on first access)
    pub fn global() -> &'static OntologyService {
        ONTOLOGY.get_or_init(|| {
            let config_dir = std::env::var("OB_POC_CONFIG_DIR")
                .unwrap_or_else(|_| "rust/config".to_string());
            Self::load(&config_dir).expect("Failed to load ontology")
        })
    }
    
    // === Entity Taxonomy ===
    
    pub fn get_entity(&self, entity_type: &str) -> Option<&EntityDef> {
        self.taxonomy.get(entity_type)
    }
    
    pub fn get_lifecycle(&self, entity_type: &str) -> Option<&EntityLifecycle> {
        self.taxonomy.get(entity_type)?.lifecycle.as_ref()
    }
    
    pub fn get_fk(&self, parent_domain: &str, child_domain: &str) -> Option<&str> {
        self.taxonomy.get_fk(parent_domain, child_domain)
    }
    
    pub fn allows_implicit_create(&self, entity_type: &str) -> bool {
        self.taxonomy.allows_implicit_create(entity_type)
    }
    
    pub fn canonical_creator(&self, entity_type: &str, subtype: Option<&str>) -> Option<String> {
        self.taxonomy.canonical_creator(entity_type, subtype)
    }
    
    // === Verb Semantics ===
    
    pub fn get_verb_lifecycle(&self, domain: &str, verb: &str) -> Option<&VerbLifecycle> {
        let runtime_verb = runtime_registry().get(domain, verb)?;
        // VerbLifecycle is loaded into RuntimeVerb from YAML
        // This requires extending RuntimeVerb to include lifecycle
        runtime_verb.lifecycle.as_ref()
    }
    
    /// Check if verb is a canonical creator for any entity type
    pub fn is_canonical_creator(&self, domain: &str, verb: &str) -> Option<&str> {
        let full_name = format!("{}.{}", domain, verb);
        for entity_type in self.taxonomy.entity_types() {
            if let Some(creator) = self.taxonomy.canonical_creator(entity_type, None) {
                if creator == full_name {
                    return Some(entity_type);
                }
            }
        }
        None
    }
}

/// Convenience function to access global ontology
pub fn ontology() -> &'static OntologyService {
    OntologyService::global()
}
```

---

## 5. Execution Plan Enhancement

### 5.1 Remove Hardcoded PARENT_FK_MAP

In `execution_plan.rs`, replace the static `PARENT_FK_MAP` with config lookup:

```rust
// REMOVE this:
// static PARENT_FK_MAP: &[(&str, &str, &str)] = &[ ... ];

// REPLACE infer_parent_fk with:
fn infer_parent_fk(parent_domain: &str, child_domain: &str) -> Option<&'static str> {
    use crate::ontology::ontology;
    ontology().get_fk(parent_domain, child_domain)
}
```

### 5.2 Add Planner Pass

Extend `compile()` to include planner logic:

```rust
// In execution_plan.rs

use crate::ontology::{ontology, OntologyService};

/// Extended compilation with planner pass
pub fn compile_with_planning(
    program: &Program,
    planning_context: &PlanningContext,
) -> Result<PlanningResult, CompileError> {
    // Phase 1: Collect verb calls
    let verb_calls: Vec<&VerbCall> = program
        .statements
        .iter()
        .filter_map(|s| match s {
            Statement::VerbCall(vc) => Some(vc),
            Statement::Comment(_) => None,
        })
        .collect();

    if verb_calls.is_empty() {
        return Ok(PlanningResult {
            plan: ExecutionPlan { steps: Vec::new() },
            synthetic_steps: Vec::new(),
            reordered: false,
            diagnostics: Vec::new(),
        });
    }

    // Phase 2: Resolve bindings and detect missing producers
    let mut binding_producers: HashMap<String, usize> = HashMap::new();
    let mut required_bindings: Vec<(usize, String, String)> = Vec::new(); // (stmt_idx, binding_name, entity_type)
    
    for (idx, vc) in verb_calls.iter().enumerate() {
        // Record what this verb produces
        if let Some(ref binding) = vc.binding {
            binding_producers.insert(binding.clone(), idx);
        }
        
        // Record what this verb consumes
        if let Some(verb_def) = runtime_registry().get(&vc.domain, &vc.verb) {
            for consume in &verb_def.consumes {
                if let Some(arg) = vc.arguments.iter().find(|a| a.key == consume.arg) {
                    if let AstNode::SymbolRef { name, .. } = &arg.value {
                        required_bindings.push((idx, name.clone(), consume.consumed_type.clone()));
                    }
                }
            }
        }
    }
    
    // Phase 3: Detect missing producers and inject synthetic creates
    let mut synthetic_statements: Vec<Statement> = Vec::new();
    let mut diagnostics: Vec<PlannerDiagnostic> = Vec::new();
    
    for (stmt_idx, binding_name, entity_type) in &required_bindings {
        if !binding_producers.contains_key(binding_name) {
            // Binding not produced by any statement
            // Check if it's in session context
            if planning_context.has_binding(binding_name) {
                continue;
            }
            
            // Check if implicit create is allowed
            let ontology = ontology();
            if ontology.allows_implicit_create(entity_type) {
                if let Some(creator_verb) = ontology.canonical_creator(entity_type, None) {
                    // Create synthetic statement
                    let synthetic = create_synthetic_verb_call(
                        &creator_verb,
                        binding_name,
                        entity_type,
                    );
                    diagnostics.push(PlannerDiagnostic::SyntheticStepInjected {
                        binding: binding_name.clone(),
                        verb: creator_verb.clone(),
                        before_stmt: *stmt_idx,
                    });
                    synthetic_statements.push(Statement::VerbCall(synthetic));
                }
            } else {
                diagnostics.push(PlannerDiagnostic::MissingProducer {
                    binding: binding_name.clone(),
                    entity_type: entity_type.clone(),
                    required_by_stmt: *stmt_idx,
                });
            }
        }
    }
    
    // Phase 4: Merge synthetic statements with original
    let mut all_statements = synthetic_statements;
    all_statements.extend(program.statements.iter().cloned());
    let merged_program = Program { statements: all_statements };
    
    // Phase 5: Topological sort with lifecycle awareness
    let sort_result = topological_sort_with_lifecycle(&merged_program, planning_context)?;
    
    // Phase 6: Compile to execution plan
    let plan = compile_sorted(&sort_result.program)?;
    
    Ok(PlanningResult {
        plan,
        synthetic_steps: diagnostics
            .iter()
            .filter_map(|d| match d {
                PlannerDiagnostic::SyntheticStepInjected { binding, verb, .. } => {
                    Some(SyntheticStep {
                        binding: binding.clone(),
                        verb: verb.clone(),
                    })
                }
                _ => None,
            })
            .collect(),
        reordered: sort_result.reordered,
        diagnostics,
    })
}

/// Create a synthetic verb call for implicit entity creation
fn create_synthetic_verb_call(
    creator_verb: &str,
    binding: &str,
    _entity_type: &str,
) -> VerbCall {
    let parts: Vec<&str> = creator_verb.split('.').collect();
    let (domain, verb) = (parts[0], parts[1]);
    
    VerbCall {
        domain: domain.to_string(),
        verb: verb.to_string(),
        arguments: vec![],  // Minimal - will need to be filled in
        binding: Some(binding.to_string()),
        span: Span::synthetic(),
    }
}
```

### 5.3 Lifecycle-Aware Topological Sort

Extend `topo_sort.rs`:

```rust
// In topo_sort.rs

use crate::ontology::{is_valid_transition, ontology};

/// Extended topological sort with lifecycle edge support
pub fn topological_sort_with_lifecycle(
    pending: &Program,
    context: &PlanningContext,
) -> Result<TopoSortResult, TopoSortError> {
    let statements = &pending.statements;
    
    if statements.is_empty() {
        return Ok(TopoSortResult {
            program: pending.clone(),
            reordered: false,
            index_map: vec![],
        });
    }

    // Build dependency graph with both binding AND lifecycle edges
    let mut deps: HashMap<usize, HashSet<usize>> = HashMap::new();
    let mut binding_to_stmt: HashMap<String, usize> = HashMap::new();
    
    // Track entity state changes: entity_binding -> (stmt_idx, new_state)
    let mut state_transitions: HashMap<String, Vec<(usize, String)>> = HashMap::new();

    // First pass: record bindings and state transitions
    for (idx, stmt) in statements.iter().enumerate() {
        deps.insert(idx, HashSet::new());
        
        if let Statement::VerbCall(vc) = stmt {
            if let Some(ref binding) = vc.binding {
                binding_to_stmt.insert(binding.clone(), idx);
            }
            
            // Check if this verb transitions entity state
            if let Some(lifecycle) = ontology().get_verb_lifecycle(&vc.domain, &vc.verb) {
                if let Some(ref to_state) = lifecycle.transitions_to {
                    // Find the entity binding from the entity_arg
                    if let Some(arg) = vc.arguments.iter().find(|a| a.key == lifecycle.entity_arg) {
                        if let AstNode::SymbolRef { name, .. } = &arg.value {
                            state_transitions
                                .entry(name.clone())
                                .or_default()
                                .push((idx, to_state.clone()));
                        }
                    }
                }
            }
        }
    }

    // Second pass: build edges from binding refs AND lifecycle requirements
    for (idx, stmt) in statements.iter().enumerate() {
        if let Statement::VerbCall(vc) = stmt {
            // Binding dependency edges (existing logic)
            for arg in &vc.arguments {
                collect_symbol_refs(&arg.value, &binding_to_stmt, context, idx, &mut deps);
            }
            
            // Lifecycle dependency edges (NEW)
            if let Some(lifecycle) = ontology().get_verb_lifecycle(&vc.domain, &vc.verb) {
                if !lifecycle.requires_states.is_empty() {
                    // This verb requires entity to be in specific state(s)
                    if let Some(arg) = vc.arguments.iter().find(|a| a.key == lifecycle.entity_arg) {
                        if let AstNode::SymbolRef { name, .. } = &arg.value {
                            // Find the statement that puts entity into required state
                            if let Some(transitions) = state_transitions.get(name) {
                                for (trans_idx, to_state) in transitions {
                                    if lifecycle.requires_states.contains(to_state) && *trans_idx != idx {
                                        // This statement depends on trans_idx completing first
                                        deps.get_mut(&idx).unwrap().insert(*trans_idx);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Kahn's algorithm (same as existing)
    // ... rest of implementation unchanged ...
}
```

---

## 6. Runtime Registry Extension

### 6.1 Add Lifecycle to RuntimeVerb

```rust
// In runtime_registry.rs

#[derive(Debug, Clone)]
pub struct RuntimeVerb {
    pub domain: String,
    pub verb: String,
    pub full_name: String,
    pub description: String,
    pub behavior: RuntimeBehavior,
    pub args: Vec<RuntimeArg>,
    pub returns: RuntimeReturn,
    pub produces: Option<VerbProduces>,
    pub consumes: Vec<VerbConsumes>,
    // NEW
    pub lifecycle: Option<VerbLifecycle>,
}
```

### 6.2 Load Lifecycle from YAML

In the `build_verb` method:

```rust
fn build_verb(domain: &str, verb: &str, config: &VerbConfig) -> RuntimeVerb {
    // ... existing code ...
    
    RuntimeVerb {
        domain: domain.to_string(),
        verb: verb.to_string(),
        full_name: format!("{}.{}", domain, verb),
        description: config.description.clone(),
        behavior,
        args,
        returns,
        produces: config.produces.clone(),
        consumes: config.consumes.clone(),
        // NEW
        lifecycle: config.lifecycle.clone(),
    }
}
```

---

## 7. Integration Points

### 7.1 Update lib.rs

```rust
// In rust/src/lib.rs, add:

pub mod ontology;
```

### 7.2 Update dsl_v2/mod.rs Exports

```rust
// Add to exports:
pub use execution_plan::{
    compile, compile_with_planning, CompileError, ExecutionPlan, ExecutionStep,
    PlanningContext, PlanningResult, PlannerDiagnostic, SyntheticStep,
};
```

### 7.3 Wire Into Execution Flows

In the REPL and direct execution paths, use `compile_with_planning`:

```rust
// Example integration in executor or session handler:

pub async fn execute_with_planning(
    source: &str,
    session_context: &SessionContext,
) -> Result<ExecutionResult, ExecutionError> {
    // Parse
    let program = parse_program(source)?;
    
    // Create planning context from session
    let planning_context = PlanningContext::from_session(session_context);
    
    // Compile with planning (includes synthetic step injection)
    let planning_result = compile_with_planning(&program, &planning_context)?;
    
    // Log synthetic steps
    for step in &planning_result.synthetic_steps {
        tracing::info!("Injected synthetic: {} for @{}", step.verb, step.binding);
    }
    
    // Execute
    execute_plan(&planning_result.plan, executor).await
}
```

---

## 8. Implementation Phases

### Phase 1: Config Foundation (Week 1)

**Goal:** Config files exist and load correctly.

- [ ] Create `rust/config/ontology/` directory
- [ ] Create `entity_taxonomy.yaml` with CBU, entity, kyc_case, document definitions
- [ ] Add types to `config/types.rs`: `EntityTaxonomyConfig`, `EntityDef`, `EntityLifecycle`, etc.
- [ ] Create `ontology/` module with `taxonomy.rs`, `lifecycle.rs`, `service.rs`
- [ ] Unit tests: taxonomy loads, FK lookup works, lifecycle validation works

**Deliverables:**
- `rust/config/ontology/entity_taxonomy.yaml`
- `rust/src/ontology/mod.rs`, `taxonomy.rs`, `lifecycle.rs`, `service.rs`
- Tests passing

### Phase 2: Remove Hardcoding (Week 1)

**Goal:** `PARENT_FK_MAP` comes from config.

- [ ] Update `execution_plan.rs` to use `ontology().get_fk()`
- [ ] Remove `PARENT_FK_MAP` static array
- [ ] Verify all existing tests pass
- [ ] Add test for FK lookup from config

**Deliverables:**
- Modified `execution_plan.rs`
- No behavior change, just source of truth moved

### Phase 3: Verb Lifecycle Extension (Week 2) ✅ COMPLETED

**Goal:** Verb YAML supports lifecycle semantics.

- [x] Add `VerbLifecycle` type to `config/types.rs`
- [x] Extend `VerbConfig` with `lifecycle` field
- [x] Extend `RuntimeVerb` with `lifecycle` field
- [x] Update `RuntimeVerbRegistry::build_verb` to load lifecycle
- [ ] Add lifecycle sections to key verbs in `cbu.yaml`, `kyc.yaml` (optional - config exists)
- [x] Unit tests: lifecycle loads correctly

**Deliverables:**
- Extended `config/types.rs`
- Extended `runtime_registry.rs`
- Updated verb YAML files

### Phase 4: Planner Pass (Week 2-3) ✅ COMPLETED

**Goal:** `compile_with_planning` detects missing producers.

- [x] Add `PlanningContext`, `PlanningResult`, `PlannerDiagnostic` types
- [x] Implement `compile_with_planning` in `execution_plan.rs`
- [x] Detect missing bindings
- [x] Check `allows_implicit_create` in taxonomy
- [x] Generate synthetic create statements (minimal args)
- [x] Return diagnostics for missing producers (no auto-create)

**Deliverables:**
- `compile_with_planning` function
- Synthetic step generation
- Diagnostic reporting

### Phase 5: Lifecycle-Aware Sort (Week 3) ✅ COMPLETED

**Goal:** Topological sort respects lifecycle transitions.

- [x] Extend `topo_sort.rs` with `topological_sort_with_lifecycle`
- [x] Add lifecycle edge detection (requires_states → transitions_to)
- [x] Integrate with `compile_with_planning`
- [x] Tests: verify ordering respects lifecycle constraints

**Deliverables:**
- Extended `topo_sort.rs`
- Lifecycle ordering tests

### Phase 6: Integration & Wiring (Week 3-4) ✅ COMPLETED

**Goal:** Planner runs in real execution paths.

- [x] Wire `compile_with_planning` into `direct_execute_dsl`
- [ ] Wire into session REPL (deferred - requires session state refactor)
- [x] Add response fields: `synthetic_steps`, `reordered`, `diagnostics`
- [x] Feature flag for opt-in (`PLANNER_ENABLED=1`)

**Deliverables:**
- Modified execution paths
- Feature flag support

### Phase 7: Testing & Documentation (Week 4) ✅ COMPLETED

**Goal:** Comprehensive tests and docs.

- [x] Integration tests:
  - Missing CBU → synthetic `cbu.create` injected
  - Missing entity → MissingProducer diagnostic
  - Out-of-order lifecycle verbs → reordered
  - Lifecycle violation → diagnostic emitted
- [ ] Update `README.md` with planner documentation (see CLAUDE.md updates below)
- [x] Document ontology config format (in entity_taxonomy.yaml)
- [x] Document verb lifecycle extension (in config/types.rs)

**Deliverables:**
- Integration test suite (19 execution_plan tests, 11 topo_sort tests)
- Documentation

---

## 9. Testing Scenarios

### 9.1 Missing Producer Injection

```lisp
; INPUT (missing @fund creation)
(cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
(entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)

; EXPECTED OUTPUT (synthetic @fund create injected)
(cbu.create :name ??? :jurisdiction ??? :as @fund)  ; SYNTHETIC
(entity.create-proper-person :first-name "John" :last-name "Smith" :as @john)
(cbu.assign-role :cbu-id @fund :entity-id @john :role "DIRECTOR")
```

### 9.2 Lifecycle Ordering

```lisp
; INPUT (out of lifecycle order)
(cbu.mark-validated :cbu-id @fund)                    ; Requires DRAFT state
(cbu.create :name "Test" :jurisdiction "US" :as @fund) ; Produces DISCOVERED state
(cbu.set-status :cbu-id @fund :status "DRAFT")        ; Transitions to DRAFT

; EXPECTED OUTPUT (reordered for lifecycle)
(cbu.create :name "Test" :jurisdiction "US" :as @fund)
(cbu.set-status :cbu-id @fund :status "DRAFT")
(cbu.mark-validated :cbu-id @fund)
```

### 9.3 Lifecycle Violation Warning

```lisp
; INPUT
(cbu.create :name "Test" :jurisdiction "US" :as @fund)
(cbu.mark-validated :cbu-id @fund)  ; ERROR: requires DRAFT, but @fund is DISCOVERED

; EXPECTED DIAGNOSTIC
; Warning: cbu.mark-validated requires @fund in states [DRAFT, PENDING_VALIDATION] 
;          but @fund will be in state DISCOVERED
```

---

## 10. Non-Goals / Out of Scope

- **DB lifecycle enforcement functions** — Not implementing `is_valid_*_transition` in Postgres
- **Automatic arg inference for synthetic steps** — Synthetic creates have placeholder args
- **Undo/rollback planning** — If step 2 fails, no automatic rollback hints
- **Cross-session lifecycle tracking** — Each session/execution is independent
- **UI integration** — API returns data; UI changes are separate work

---

## 11. Success Criteria

1. **Config loads without errors** — `OntologyService::global()` works
2. **FK lookup works** — `execution_plan.rs` uses config, not hardcoded map
3. **Verb lifecycle loads** — `runtime_registry().get("cbu", "mark-validated").lifecycle` is Some
4. **Missing producers detected** — Diagnostic emitted for unbound `@refs`
5. **Synthetic injection works** — When allowed, create statements appear in plan
6. **Lifecycle ordering works** — Out-of-order verbs get reordered
7. **Existing tests pass** — No regression in current functionality

---

> **Implementation Note for Claude-Code:**
>
> Start with Phase 1 (config foundation) and Phase 2 (remove hardcoding). 
> These have zero behavior change but establish the config-driven pattern.
> Then proceed through phases 3-7 incrementally.
>
> Always run `cargo test` after each phase to catch regressions.
