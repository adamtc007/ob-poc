# Data Meta-Model Philosophy

## The Problem with Entity-Oriented Design

Traditional enterprise software creates **artificial splits along entity lines**:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    TRADITIONAL APPROACH (Anti-Pattern)                       │
│                                                                              │
│   ProductService.java          InstrumentService.java                       │
│   ├── getProducts()            ├── getInstruments()                         │
│   ├── getServicesForProduct()  ├── getLifecyclesForInstrument()             │
│   ├── getResourcesForService() ├── getResourcesForLifecycle()               │
│   ├── provisionResource()      ├── provisionResource()      ← DUPLICATE     │
│   ├── analyzeGaps()            ├── analyzeGaps()            ← DUPLICATE     │
│   └── checkReadiness()         └── checkReadiness()         ← DUPLICATE     │
│                                                                              │
│   EntityService.java           DocumentService.java                          │
│   ├── getEntities()            ├── getDocuments()                            │
│   ├── getVerificationsFor...   ├── getPipelinesFor...                        │
│   ├── getEvidenceFor...        ├── getExtractionsFor...                      │
│   ├── provisionEvidence()      ├── provisionExtraction()   ← DUPLICATE      │
│   ├── analyzeGaps()            ├── analyzeGaps()           ← DUPLICATE      │
│   └── checkReadiness()         └── checkReadiness()        ← DUPLICATE      │
└─────────────────────────────────────────────────────────────────────────────┘
```

**What happens:**
- 4 domains × same pattern = 4× the code
- Each team "owns" their entity, reimplements the same logic
- Bugs get fixed in one place, not others
- Subtle divergence over time
- New developer: "Why are there 4 ways to do gap analysis?"

**This is what kills enterprise software projects.**

---

## The Reality: Data Patterns Are the Same

Strip away the entity names. Look at the **structure**:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         STRUCTURAL VIEW                                      │
│                                                                              │
│   Table A (types)                                                            │
│   ├── id: uuid                                                               │
│   ├── code: string                                                           │
│   ├── name: string                                                           │
│   └── category: string                                                       │
│            │                                                                 │
│            │ M:N junction                                                    │
│            ▼                                                                 │
│   Table B (operations)                                                       │
│   ├── id: uuid                                                               │
│   ├── code: string                                                           │
│   ├── name: string                                                           │
│   └── owner: string                                                          │
│            │                                                                 │
│            │ M:N junction                                                    │
│            ▼                                                                 │
│   Table C (resource_types)                                                   │
│   ├── id: uuid                                                               │
│   ├── code: string                                                           │
│   ├── provisioning_verb: string                                              │
│   └── depends_on: jsonb                                                      │
│            │                                                                 │
│            │ FK per CBU                                                      │
│            ▼                                                                 │
│   Table D (instances)                                                        │
│   ├── id: uuid                                                               │
│   ├── cbu_id: uuid                                                           │
│   ├── resource_type_id: uuid                                                 │
│   ├── context (market/currency/counterparty)                                 │
│   └── status: string                                                         │
└─────────────────────────────────────────────────────────────────────────────┘
```

**This is the same structure whether you call it:**
- Product → Service → Resource
- Instrument → Lifecycle → Resource
- Entity → Verification → Evidence
- Document → Pipeline → Extraction

The **bits and bytes are identical**. Only the **metadata differs**.

---

## The Solution: Structure + Metadata

### Think in Structures

```rust
// THE structure - one implementation
struct TaxonomyChain {
    type_table: &'static str,
    type_id_col: &'static str,
    type_code_col: &'static str,
    
    op_table: &'static str,
    op_id_col: &'static str,
    op_code_col: &'static str,
    
    resource_table: &'static str,
    resource_id_col: &'static str,
    
    type_op_junction: &'static str,
    op_resource_junction: &'static str,
    
    instance_table: &'static str,
    gap_view: &'static str,
}
```

### Pivot Functionality on Metadata

```rust
impl TaxonomyChain {
    // ONE implementation - works for ALL domains
    fn discover(&self, pool: &PgPool, type_code: &str) -> Result<Discovery> {
        let query = format!(
            "SELECT o.* FROM {} o 
             JOIN {} j ON j.{} = o.{}
             JOIN {} t ON t.{} = j.{}
             WHERE t.{} = $1",
            self.op_table,
            self.type_op_junction,
            self.op_id_col, self.op_id_col,
            self.type_table,
            self.type_id_col, self.type_id_col,
            self.type_code_col
        );
        // Execute with type_code parameter
    }
    
    fn analyze_gaps(&self, pool: &PgPool, cbu_id: Uuid) -> Result<Vec<Gap>> {
        let query = format!("SELECT * FROM {} WHERE cbu_id = $1", self.gap_view);
        // Execute
    }
    
    fn provision(&self, pool: &PgPool, args: ProvisionArgs) -> Result<Uuid> {
        let query = format!(
            "INSERT INTO {} (cbu_id, resource_type_id, ...) VALUES ($1, $2, ...)",
            self.instance_table
        );
        // Execute
    }
}
```

### Instantiate with Metadata

```rust
// Product domain - just metadata
const PRODUCT_TAXONOMY: TaxonomyChain = TaxonomyChain {
    type_table: "products",
    type_id_col: "product_id",
    type_code_col: "product_code",
    op_table: "services",
    op_id_col: "service_id",
    op_code_col: "service_code",
    resource_table: "service_resource_types",
    resource_id_col: "resource_id",
    type_op_junction: "product_services",
    op_resource_junction: "service_resource_capabilities",
    instance_table: "cbu_resource_instances",
    gap_view: "v_cbu_service_gaps",
};

// Instrument domain - just metadata
const INSTRUMENT_TAXONOMY: TaxonomyChain = TaxonomyChain {
    type_table: "custody.instrument_classes",
    type_id_col: "class_id",
    type_code_col: "code",
    op_table: "lifecycles",
    op_id_col: "lifecycle_id",
    op_code_col: "code",
    resource_table: "lifecycle_resource_types",
    resource_id_col: "resource_type_id",
    type_op_junction: "instrument_lifecycles",
    op_resource_junction: "lifecycle_resource_capabilities",
    instance_table: "cbu_lifecycle_instances",
    gap_view: "v_cbu_lifecycle_gaps",
};

// Usage - SAME code, different metadata
PRODUCT_TAXONOMY.discover(&pool, "GLOBAL_CUSTODY").await?;
INSTRUMENT_TAXONOMY.discover(&pool, "IRS").await?;
```

---

## Why This Works

### 1. Bits and Bytes Are Real

Entity names are human abstractions. At the machine level:
- A UUID is 16 bytes whether it's a `product_id` or `lifecycle_id`
- A string is a string whether it's called `code` or `name`
- A junction table is a junction table

**Design for the machine, parameterize for humans.**

### 2. Metadata Is Data

The table names, column names, relationships - these are just data:
```rust
const METADATA: &str = r#"
{
  "type_table": "products",
  "op_table": "services",
  ...
}
"#;
```

Could be JSON, YAML, or Rust consts. Doesn't matter. It's data that configures behavior.

### 3. One Bug Fix, All Domains

When you fix a bug in `discover()`:
- Traditional: Fix in ProductService, forget InstrumentService, EntityService breaks in production
- Meta-model: Fix once, all domains get the fix

### 4. New Domain = Configuration

Adding Document processing:
```rust
const DOCUMENT_TAXONOMY: TaxonomyChain = TaxonomyChain {
    type_table: "document_types",
    op_table: "processing_pipelines",
    // ... 10 more lines of metadata
};
```

No new logic. No new code paths. Just metadata.

---

## The DSL Connection

The DSL is the **human-facing metadata layer**:

```
lifecycle.discover instrument-class:IRS
product.discover product-code:GLOBAL_CUSTODY
entity.discover entity-type:LIMITED_COMPANY
```

The NOM parser doesn't care about the domain. It parses:
```
{domain}.{verb} {arg}:{value}
```

The executor looks up the domain's metadata and calls the generic implementation.

```rust
fn execute(domain: &str, verb: &str, args: &Args) -> Result<Value> {
    let taxonomy = match domain {
        "lifecycle" => &INSTRUMENT_TAXONOMY,
        "product" => &PRODUCT_TAXONOMY,
        "entity" => &ENTITY_TAXONOMY,
        _ => bail!("Unknown domain"),
    };
    
    match verb {
        "discover" => taxonomy.discover(pool, args.get("code")?).await,
        "analyze-gaps" => taxonomy.analyze_gaps(pool, args.get("cbu_id")?).await,
        "provision" => taxonomy.provision(pool, args.into()).await,
        _ => bail!("Unknown verb"),
    }
}
```

---

## Contrast: OOP vs Data-Oriented

### Object-Oriented (Entity-Focused)

```java
// Each entity is special
class Product extends BaseEntity { ... }
class Instrument extends BaseEntity { ... }
class Entity extends BaseEntity { ... }

// Each service is special
class ProductService { 
    void discover(Product p) { /* product-specific */ }
}
class InstrumentService {
    void discover(Instrument i) { /* instrument-specific */ }
}
```

**Problem:** Polymorphism creates N implementations hiding behind an interface. Bugs hide. Behavior diverges.

### Data-Oriented (Structure-Focused)

```rust
// ONE structure
struct TaxonomyChain { ... }

// ONE implementation
impl TaxonomyChain {
    fn discover(&self, ...) { /* ONE implementation */ }
}

// N configurations (just data)
const PRODUCT: TaxonomyChain = TaxonomyChain { ... };
const INSTRUMENT: TaxonomyChain = TaxonomyChain { ... };
```

**Benefit:** One implementation. N configurations. Bugs get fixed everywhere. Behavior is consistent.

---

## The Meta-Model Principle

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                              │
│   "Think in bits and bytes / structures - then pivot functionality          │
│    on metadata"                                                              │
│                                                                              │
│   STRUCTURE (fixed)              METADATA (variable)                         │
│   ┌─────────────────┐            ┌─────────────────┐                        │
│   │ type_table      │◄───────────│ "products"      │                        │
│   │ op_table        │◄───────────│ "services"      │                        │
│   │ resource_table  │◄───────────│ "resource_types"│                        │
│   │ instance_table  │◄───────────│ "cbu_instances" │                        │
│   └─────────────────┘            └─────────────────┘                        │
│          │                                                                   │
│          ▼                                                                   │
│   BEHAVIOR (derived from structure + metadata)                               │
│   ┌─────────────────────────────────────────────┐                           │
│   │ discover()   - ONE implementation            │                           │
│   │ analyze_gaps() - ONE implementation          │                           │
│   │ provision()  - ONE implementation            │                           │
│   └─────────────────────────────────────────────┘                           │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Application to ob-poc

| Layer | What It Is | How It's Expressed |
|-------|------------|-------------------|
| **Structure** | TaxonomyChain | Rust struct/trait |
| **Metadata** | Table names, columns, relationships | YAML config |
| **Behavior** | discover, analyze_gaps, provision | Generic impl |
| **Interface** | DSL verbs | NOM parser |

The entire system is:
1. **Schema** - structural pattern repeated per domain
2. **YAML** - metadata configuring each domain
3. **Rust** - ONE generic implementation
4. **DSL** - human interface to the generic implementation

---

## Summary

> **"This is a sophisticated data meta-model - but for a reason. It prevents artificial splits along 'entity lines' when in reality the data 'patterns' are the same - it's what kills other entity model data implementations."**

The reason enterprise software fails:
- Developers think in **entities** (Product, Customer, Order)
- They create **separate implementations** for each
- The implementations **diverge** over time
- **Complexity explodes** with N × M code paths

The solution:
- Think in **structures** (what's the actual data shape?)
- Identify **patterns** (it's all Type → Op → Resource)
- Build **ONE implementation** parameterized by metadata
- **Complexity stays constant** regardless of domain count

This is why ob-poc uses:
- Common taxonomy model
- YAML-driven configuration
- Generic Rust traits
- NOM-parseable DSL

One pattern. Multiple instances. Concentration of implementation.
