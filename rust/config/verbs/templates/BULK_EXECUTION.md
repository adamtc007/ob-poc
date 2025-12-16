# Bulk Execution Pattern

## Overview

Bulk execution allows expanding a template N times from a database query, then executing all generated DSL as a batch.

## Pattern: Query → Expand × N → Batch Execute

```
┌─────────────────────────────────────────────────────────────────┐
│  1. QUERY: Find entities to process                             │
│     "SELECT entity_id, name FROM entities                       │
│      WHERE name ILIKE 'Allianz%' AND type = 'fund_subfund'"     │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  2. CONTEXT: Define shared parameters                           │
│     manco_name: "Allianz Global Investors Luxembourg S.A."      │
│     im_name: "Allianz Global Investors GmbH"                    │
│     jurisdiction: "LU"                                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  3. EXPAND × N: For each query row, expand template             │
│     Row 1: { fund_name: "Allianz AI Income", fund_entity_id: ...}│
│     Row 2: { fund_name: "Allianz Balanced", fund_entity_id: ... }│
│     ...                                                          │
│     → N DSL blocks generated                                     │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  4. BATCH EXECUTE: Run all DSL in single transaction            │
│     - Parse all blocks                                          │
│     - Resolve all entity references                             │
│     - Execute in dependency order                               │
│     - Rollback on any failure                                   │
└─────────────────────────────────────────────────────────────────┘
```

## Example: Onboard All Allianz Funds

### Step 1: Query

```sql
SELECT e.entity_id, e.name 
FROM "ob-poc".entities e
JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
WHERE e.name ILIKE 'Allianz%' AND et.type_code = 'fund_subfund'
ORDER BY e.name
LIMIT 10;
```

### Step 2: Shared Context

```yaml
shared:
  manco_name: "Allianz Global Investors Luxembourg S.A."
  im_name: "Allianz Global Investors GmbH"
  jurisdiction: "LU"
```

### Step 3: Template

```yaml
template: onboard-fund-cbu
body: |
  (cbu.ensure :name "$fund_name" :jurisdiction "$jurisdiction" :client-type "FUND" :as @cbu)
  (cbu.assign-role :cbu-id @cbu :entity-id "$fund_entity_id" :role "ASSET_OWNER")
  (cbu.assign-role :cbu-id @cbu :entity-id "$manco_name" :role "MANAGEMENT_COMPANY")
  (cbu.assign-role :cbu-id @cbu :entity-id "$im_name" :role "INVESTMENT_MANAGER")
```

### Step 4: Expanded DSL (for 10 funds)

```clojure
;; Fund 1: Allianz AI Income
(cbu.ensure :name "Allianz AI Income" :jurisdiction "LU" :client-type "FUND" :as @cbu-1)
(cbu.assign-role :cbu-id @cbu-1 :entity-id "ae6eb4d6-d7a8-4fed-b075-b0c1231246d5" :role "ASSET_OWNER")
(cbu.assign-role :cbu-id @cbu-1 :entity-id "Allianz Global Investors Luxembourg S.A." :role "MANAGEMENT_COMPANY")
(cbu.assign-role :cbu-id @cbu-1 :entity-id "Allianz Global Investors GmbH" :role "INVESTMENT_MANAGER")

;; Fund 2: Allianz ActiveInvest Balanced
(cbu.ensure :name "Allianz ActiveInvest Balanced" :jurisdiction "LU" :client-type "FUND" :as @cbu-2)
(cbu.assign-role :cbu-id @cbu-2 :entity-id "fadb3423-27e4-41d0-9e90-2c8e449feda2" :role "ASSET_OWNER")
(cbu.assign-role :cbu-id @cbu-2 :entity-id "Allianz Global Investors Luxembourg S.A." :role "MANAGEMENT_COMPANY")
(cbu.assign-role :cbu-id @cbu-2 :entity-id "Allianz Global Investors GmbH" :role "INVESTMENT_MANAGER")

;; ... 8 more funds
```

## Implementation Components

### 1. BulkExpander (new module)

```rust
pub struct BulkExpander {
    template_registry: Arc<TemplateRegistry>,
}

impl BulkExpander {
    /// Expand template for each row from query results
    pub fn expand_from_query(
        &self,
        template_id: &str,
        query_rows: Vec<HashMap<String, Value>>,
        shared_context: HashMap<String, String>,
    ) -> Result<Vec<ExpansionResult>, TemplateError>;
}
```

### 2. BatchExecutor (extension to DslExecutor)

```rust
impl DslExecutor {
    /// Execute multiple DSL blocks in a single transaction
    pub async fn execute_batch(
        &self,
        dsl_blocks: Vec<String>,
    ) -> Result<BatchExecutionResult, ExecutionError>;
}
```

### 3. CLI Command

```bash
# Bulk execute from template + query
dsl_cli bulk \
  --template onboard-fund-cbu \
  --query "SELECT entity_id as fund_entity_id, name as fund_name FROM entities WHERE ..." \
  --context manco_name="Allianz Global Investors Luxembourg S.A." \
  --context im_name="Allianz Global Investors GmbH" \
  --context jurisdiction=LU \
  --dry-run
```

### 4. API Endpoint

```
POST /api/bulk/execute
{
  "template_id": "onboard-fund-cbu",
  "query": "SELECT ... FROM ...",
  "shared_context": {
    "manco_name": "...",
    "im_name": "...",
    "jurisdiction": "LU"
  },
  "dry_run": false
}
```

## Entity Resolution

The key insight is that entity names in templates (like `$manco_name`) get resolved via the existing enrichment pipeline:

1. Template expansion produces plain strings
2. Parser creates AST with string literals
3. Enrichment converts strings to `EntityRef` based on verb YAML lookup config
4. Semantic validator resolves `EntityRef` → UUID via EntityGateway
5. Executor uses resolved UUIDs

This means bulk execution gets the same entity resolution as single-statement execution.

## Benefits

1. **Declarative**: Template defines what, query defines scope
2. **Transactional**: All-or-nothing execution
3. **Auditable**: Each expansion is traceable DSL
4. **Reusable**: Same template for 1 or 1000 entities
5. **Validated**: Full DSL pipeline (parse, lint, resolve) before execution
