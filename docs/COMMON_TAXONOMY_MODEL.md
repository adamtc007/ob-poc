# Common Taxonomy Model

## Principle

**One pattern, multiple instances.** Every complex domain in ob-poc follows the same three-tier taxonomy structure:

```
WHAT → OPERATIONS → RESOURCES
  │         │           │
  │         │           └─► Things operations need (accounts, connections, agreements)
  │         └─────────────► Services/processes that run (settlement, pricing, confirmation)
  └───────────────────────► Domain objects being managed (products, instruments, entities)
```

This is how we manage complexity: **concentration of implementation** with **shared patterns**.

---

## The Pattern

### Structure

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         TAXONOMY PATTERN                                     │
│                                                                              │
│   Domain Object ──(M:N)──► Operation ──(M:N)──► Resource Type               │
│        │                      │                      │                       │
│   [domain]_types         [domain]_ops         [domain]_resource_types       │
│        │                      │                      │                       │
│        └── [domain]_type_ops ─┘                      │                       │
│                               └── [domain]_op_resources ─┘                   │
│                                                      │                       │
│                                        [domain]_instances (per CBU)          │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Tables (Generic)

| Table | Purpose | Cardinality |
|-------|---------|-------------|
| `{domain}_types` | What exists in this domain | Reference data |
| `{domain}_ops` | What operations run | Reference data |
| `{domain}_resource_types` | What resources ops need | Reference data |
| `{domain}_type_ops` | Which ops for which types | Junction (M:N) |
| `{domain}_op_resources` | Which resources for which ops | Junction (M:N) |
| `cbu_{domain}_instances` | Provisioned resources per CBU | Instance data |

### Verbs (Generic)

| Verb Pattern | Purpose |
|--------------|---------|
| `{domain}.read` | Read type/op/resource definition |
| `{domain}.list` | List with filters |
| `{domain}.list-ops-for-type` | Get operations for a domain type |
| `{domain}.list-resources-for-op` | Get resources for an operation |
| `{domain}.provision` | Provision resource instance for CBU |
| `{domain}.analyze-gaps` | Find missing resources |
| `{domain}.check-readiness` | Check if CBU is ready |
| `{domain}.discover` | Full discovery for a type |

### Views (Generic)

| View | Purpose |
|------|---------|
| `v_cbu_{domain}_coverage` | Coverage status per CBU |
| `v_cbu_{domain}_gaps` | Missing resources per CBU |

---

## Current Instances

### Instance 1: Product Domain

```
Product → Service → Service Resource
```

| Generic | Product Instance |
|---------|------------------|
| `{domain}_types` | `products` |
| `{domain}_ops` | `services` |
| `{domain}_resource_types` | `service_resource_types` |
| `{domain}_type_ops` | `product_services` |
| `{domain}_op_resources` | `service_resource_capabilities` |
| `cbu_{domain}_instances` | `cbu_resource_instances` |

**Example:** 
- Product: "Global Custody"
- Service: "Settlement", "Corporate Actions", "Pricing"
- Resource: "SWIFT Connection", "Bloomberg Feed", "CA Platform"

### Instance 2: Instrument Domain

```
Instrument Class → Lifecycle → Lifecycle Resource
```

| Generic | Instrument Instance |
|---------|---------------------|
| `{domain}_types` | `instrument_classes` |
| `{domain}_ops` | `lifecycles` |
| `{domain}_resource_types` | `lifecycle_resource_types` |
| `{domain}_type_ops` | `instrument_lifecycles` |
| `{domain}_op_resources` | `lifecycle_resource_capabilities` |
| `cbu_{domain}_instances` | `cbu_lifecycle_instances` |

**Example:**
- Instrument: "IRS" (Interest Rate Swap)
- Lifecycle: "Confirmation", "Collateral VM", "Reset Fixing"
- Resource: "ISDA Agreement", "CSA", "MarkitWire Link"

---

## Future Instances (Potential)

### Instance 3: Entity Domain

```
Entity Type → Verification → Evidence Type
```

| Generic | Entity Instance |
|---------|-----------------|
| `{domain}_types` | `entity_types` |
| `{domain}_ops` | `verification_processes` |
| `{domain}_resource_types` | `evidence_types` |
| `{domain}_type_ops` | `entity_verifications` |
| `{domain}_op_resources` | `verification_evidence_requirements` |
| `cbu_{domain}_instances` | `entity_evidence_instances` |

**Example:**
- Entity Type: "Limited Company"
- Verification: "KYC", "UBO Discovery", "Sanctions Screening"
- Evidence: "Certificate of Incorporation", "Register Extract", "Proof of Address"

### Instance 4: Document Domain

```
Document Type → Processing → Extraction Type
```

| Generic | Document Instance |
|---------|-------------------|
| `{domain}_types` | `document_types` |
| `{domain}_ops` | `processing_pipelines` |
| `{domain}_resource_types` | `extraction_types` |
| `{domain}_type_ops` | `document_processing` |
| `{domain}_op_resources` | `pipeline_extractions` |
| `cbu_{domain}_instances` | `document_extraction_instances` |

**Example:**
- Document Type: "Passport"
- Processing: "OCR", "Face Match", "MRZ Validation"
- Extraction: "Name", "DOB", "Nationality", "Expiry"

---

## Implementation Pattern

### 1. Schema Template

```sql
-- Types (reference data)
CREATE TABLE "{schema}".{domain}_types (
    {domain}_type_id uuid PRIMARY KEY,
    code varchar(50) UNIQUE NOT NULL,
    name varchar(255) NOT NULL,
    category varchar(100),
    -- domain-specific columns
    is_active boolean DEFAULT true
);

-- Operations (reference data)
CREATE TABLE "{schema}".{domain}_ops (
    {domain}_op_id uuid PRIMARY KEY,
    code varchar(50) UNIQUE NOT NULL,
    name varchar(255) NOT NULL,
    category varchar(100),
    owner varchar(100),
    -- domain-specific columns
    is_active boolean DEFAULT true
);

-- Resource Types (reference data)
CREATE TABLE "{schema}".{domain}_resource_types (
    resource_type_id uuid PRIMARY KEY,
    code varchar(50) UNIQUE NOT NULL,
    name varchar(255) NOT NULL,
    resource_type varchar(100),
    provisioning_verb varchar(100),
    -- scoping
    per_market boolean DEFAULT false,
    per_currency boolean DEFAULT false,
    per_counterparty boolean DEFAULT false,
    -- dependencies
    depends_on jsonb,
    is_active boolean DEFAULT true
);

-- Type → Op junction
CREATE TABLE "{schema}".{domain}_type_ops (
    id uuid PRIMARY KEY,
    {domain}_type_id uuid NOT NULL REFERENCES {domain}_types,
    {domain}_op_id uuid NOT NULL REFERENCES {domain}_ops,
    is_mandatory boolean DEFAULT true,
    display_order integer DEFAULT 100,
    UNIQUE ({domain}_type_id, {domain}_op_id)
);

-- Op → Resource junction
CREATE TABLE "{schema}".{domain}_op_resources (
    id uuid PRIMARY KEY,
    {domain}_op_id uuid NOT NULL REFERENCES {domain}_ops,
    resource_type_id uuid NOT NULL REFERENCES {domain}_resource_types,
    is_required boolean DEFAULT true,
    priority integer DEFAULT 100,
    UNIQUE ({domain}_op_id, resource_type_id)
);

-- CBU Instances
CREATE TABLE "{schema}".cbu_{domain}_instances (
    instance_id uuid PRIMARY KEY,
    cbu_id uuid NOT NULL REFERENCES cbus,
    resource_type_id uuid NOT NULL REFERENCES {domain}_resource_types,
    instance_url varchar(500) UNIQUE,
    -- context scoping
    market_id uuid,
    currency varchar(3),
    counterparty_entity_id uuid,
    -- status
    status varchar(50) DEFAULT 'PENDING',
    -- provider
    provider_code varchar(50),
    provider_config jsonb,
    -- lifecycle
    created_at timestamptz DEFAULT now(),
    updated_at timestamptz DEFAULT now()
);

-- Gap View
CREATE VIEW "{schema}".v_cbu_{domain}_gaps AS
SELECT 
    -- join through the chain to find missing resources
    ...
```

### 2. YAML Template

```yaml
# config/ontology/{domain}_taxonomy.yaml

version: "1.0"
description: "{Domain} taxonomy"

{domain}_ops:
  - code: OP_CODE
    name: "Operation Name"
    category: CATEGORY
    owner: OWNER

{domain}_resource_types:
  - code: RESOURCE_CODE
    name: "Resource Name"
    resource_type: TYPE
    provisioning_verb: domain.provision
    per_market: false
    per_counterparty: true
    depends_on: [OTHER_RESOURCE]

{domain}_type_ops:
  TYPE_CODE:
    mandatory: [OP_1, OP_2]
    optional: [OP_3]

{domain}_op_resources:
  OP_CODE:
    required: [RESOURCE_1, RESOURCE_2]
    optional: [RESOURCE_3]
```

### 3. Verb Template

```yaml
# config/verbs/{domain}.yaml

domains:
  {domain}:
    verbs:
      read:
        behavior: crud
        crud:
          operation: select
          table: {domain}_ops
          
      list-ops-for-type:
        behavior: crud
        crud:
          operation: select_with_join
          primary_table: {domain}_ops
          join_table: {domain}_type_ops
          
      provision:
        behavior: plugin
        plugin:
          handler: {Domain}ProvisionOp
          
      analyze-gaps:
        behavior: plugin
        plugin:
          handler: {Domain}AnalyzeGapsOp
```

### 4. Rust Template

```rust
// Generic trait for taxonomy domains
pub trait TaxonomyDomain {
    type TypeRecord: FromRow;
    type OpRecord: FromRow;
    type ResourceTypeRecord: FromRow;
    type InstanceRecord: FromRow;
    type GapRecord: FromRow;
    
    fn type_table() -> &'static str;
    fn op_table() -> &'static str;
    fn resource_type_table() -> &'static str;
    fn type_op_junction() -> &'static str;
    fn op_resource_junction() -> &'static str;
    fn instance_table() -> &'static str;
    fn gap_view() -> &'static str;
}

// Generic operations
pub struct TaxonomyOps<D: TaxonomyDomain> {
    _domain: PhantomData<D>,
}

impl<D: TaxonomyDomain> TaxonomyOps<D> {
    pub async fn discover(&self, pool: &PgPool, type_code: &str) -> Result<Discovery> {
        // Generic discovery logic using D::type_table(), D::op_table(), etc.
    }
    
    pub async fn analyze_gaps(&self, pool: &PgPool, cbu_id: Uuid) -> Result<Vec<D::GapRecord>> {
        sqlx::query_as(&format!("SELECT * FROM {} WHERE cbu_id = $1", D::gap_view()))
            .bind(cbu_id)
            .fetch_all(pool)
            .await
    }
    
    pub async fn provision(&self, pool: &PgPool, args: ProvisionArgs) -> Result<Uuid> {
        // Generic provisioning logic
    }
}

// Instrument domain implementation
pub struct InstrumentDomain;

impl TaxonomyDomain for InstrumentDomain {
    type TypeRecord = InstrumentClass;
    type OpRecord = Lifecycle;
    type ResourceTypeRecord = LifecycleResourceType;
    type InstanceRecord = CbuLifecycleInstance;
    type GapRecord = LifecycleGap;
    
    fn type_table() -> &'static str { "custody.instrument_classes" }
    fn op_table() -> &'static str { "\"ob-poc\".lifecycles" }
    fn resource_type_table() -> &'static str { "\"ob-poc\".lifecycle_resource_types" }
    fn type_op_junction() -> &'static str { "\"ob-poc\".instrument_lifecycles" }
    fn op_resource_junction() -> &'static str { "\"ob-poc\".lifecycle_resource_capabilities" }
    fn instance_table() -> &'static str { "\"ob-poc\".cbu_lifecycle_instances" }
    fn gap_view() -> &'static str { "\"ob-poc\".v_cbu_lifecycle_gaps" }
}

// Product domain implementation
pub struct ProductDomain;

impl TaxonomyDomain for ProductDomain {
    // ... similar implementation
}
```

---

## Benefits

1. **Learn once, apply everywhere** - Same mental model across domains
2. **Consistent verbs** - `domain.discover`, `domain.analyze-gaps`, `domain.provision`
3. **Consistent views** - `v_cbu_{domain}_gaps` always shows missing resources
4. **Reusable Rust code** - Generic `TaxonomyOps<D>` works for any domain
5. **Agent-friendly** - Same conversation pattern for any domain
6. **Predictable schema** - New domains follow same table structure

---

## Adding a New Domain

1. **Define the three tiers:**
   - What are the "types" (things being managed)?
   - What are the "operations" (processes that run)?
   - What are the "resources" (things operations need)?

2. **Create schema migration** using template above

3. **Create YAML taxonomy** with types, ops, resources, and junctions

4. **Create verb definitions** using template patterns

5. **Implement `TaxonomyDomain` trait** for Rust type

6. **Register in plugin handler map**

7. **Seed reference data**

---

## Cross-Domain Relationships

Domains can reference each other:

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Product   │────►│   Service   │────►│  Resource   │
└─────────────┘     └─────────────┘     └─────────────┘
       │                   │                   │
       │                   │                   │
       ▼                   ▼                   ▼
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│ Instrument  │────►│  Lifecycle  │────►│  Resource   │◄─── SHARED
└─────────────┘     └─────────────┘     └─────────────┘
       │                   │                   │
       │                   │                   │
       ▼                   ▼                   ▼
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   Entity    │────►│Verification │────►│  Evidence   │
└─────────────┘     └─────────────┘     └─────────────┘
```

**Shared resources** can be referenced across domains:
- "SWIFT Connection" might be needed by both Product services and Instrument lifecycles
- "Bloomberg Feed" serves both pricing service and pricing lifecycle
- Resource instances are provisioned once, referenced by multiple domains

---

## Summary

The **Common Taxonomy Model** is:

```
WHAT you're managing → OPERATIONS it needs → RESOURCES operations require
```

Applied consistently:
- Same table structure
- Same verb patterns  
- Same gap analysis views
- Same Rust trait implementation

This is **concentration of implementation** through **shared patterns**.

---

## CBU-Centric Application

**See also:** `docs/CBU_TRADING_MATRIX_ARCHITECTURE.md`

The CBU is the unit of complexity. It has two independent taxonomy instances:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                   CBU                                        │
│                                                                              │
│   TRADING MATRIX (independent)        PRODUCT SUBSCRIPTIONS (overlay)        │
│   ┌───────────────────────────┐       ┌───────────────────────────┐         │
│   │ Instrument → Lifecycle    │       │ Product → Service         │         │
│   │           → Resource      │       │        → Resource         │         │
│   └───────────────────────────┘       └───────────────────────────┘         │
│            │                                      │                          │
│            │  defines what they trade             │  adds service attributes │
│            ▼                                      ▼                          │
│   ┌─────────────────────────────────────────────────────────────────┐       │
│   │                   UNIFIED GAP ANALYSIS                           │       │
│   │  Shows resources needed from BOTH domains                        │       │
│   └─────────────────────────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Key principle:** The Trading Matrix is independent of products. Products ADD attributes to the matrix, they don't define it.

This enables:
- Define trading universe without committing to products
- Add/remove products without changing the matrix
- Multiple products compose their attributes onto the same matrix entries
- Unified gap analysis across both domains
