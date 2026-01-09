# Verb Definition Specification (YAML)

> CRITICAL: Verb YAML files are parsed into Rust structs via serde.
> Incorrect structure = silent deserialization failure = verb doesn't load.
> Always follow this spec exactly.

---

## 1. File Structure

```yaml
# rust/config/verbs/{domain}.yaml
#
# One file per domain. File name determines default domain if 'domains' key missing.

domains:
  {domain_name}:               # e.g., "cbu", "kyc", "investor"
    description: "..."         # Required: domain description
    verbs:
      {verb_name}:             # e.g., "create", "ensure", "approve-kyc"
        # ... verb definition
```

---

## 2. Verb Definition Structure

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     VERB DEFINITION (VerbConfig struct)                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  {verb_name}:                                                               │
│    description: string           # REQUIRED                                 │
│    behavior: crud | plugin       # REQUIRED - determines execution path    │
│                                                                              │
│    # For behavior: crud                                                      │
│    crud:                         # REQUIRED if behavior=crud                │
│      operation: ...              # REQUIRED                                 │
│      table: ...                  # Usually required                         │
│      schema: ...                 # Usually required                         │
│      ...                                                                    │
│                                                                              │
│    # For behavior: plugin                                                    │
│    handler: "HandlerName"        # REQUIRED if behavior=plugin              │
│                                                                              │
│    # Arguments                                                               │
│    args: []                      # List of ArgConfig                        │
│                                                                              │
│    # Return value                                                            │
│    returns:                      # Optional                                 │
│      type: uuid | record | affected                                         │
│      name: column_name           # For uuid return                          │
│      capture: true | false       # Can use :as @binding                     │
│                                                                              │
│    # Dataflow (for DAG ordering)                                            │
│    produces:                     # Optional - what binding type this creates│
│      type: cbu | entity | case | ...                                        │
│      subtype: proper_person | limited_company | ...                         │
│      resolved: true | false      # true = lookup, false = create            │
│                                                                              │
│    consumes: []                  # Optional - required bindings             │
│                                                                              │
│    # Lifecycle (state machine constraints)                                  │
│    lifecycle:                    # Optional                                 │
│      entity_arg: ...                                                        │
│      requires_states: []                                                    │
│      transitions_to: ...                                                    │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Field Reference

### 3.1 Top-Level Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `description` | string | **YES** | Human-readable description |
| `behavior` | enum | **YES** | `crud` or `plugin` |
| `crud` | object | if behavior=crud | CRUD configuration |
| `handler` | string | if behavior=plugin | Plugin handler name |
| `args` | list | no | List of argument definitions |
| `returns` | object | no | Return value configuration |
| `produces` | object | no | Dataflow: what binding type this creates |
| `consumes` | list | no | Dataflow: required input bindings |
| `lifecycle` | object | no | State machine constraints |

### 3.2 behavior Field

```yaml
# MUST be one of these exact values (lowercase)
behavior: crud     # Generic CRUD executor
behavior: plugin   # Custom Rust handler
```

**Common mistake:** Using `behavior: custom` or `behavior: handler` - these are invalid.

### 3.3 crud Field

```yaml
crud:
  operation: upsert              # REQUIRED - see valid operations below
  table: cbus                    # Usually required - target table
  schema: ob-poc                 # Usually required - schema name
  returning: cbu_id              # Column to return (for insert/upsert)
  key: cbu_id                    # Primary key column (for update/delete)
  conflict_keys:                 # For upsert - columns for ON CONFLICT
    - name
    - jurisdiction
  set_values:                    # For update - hardcoded values
    status: "APPROVED"
```

**Valid `operation` values:**

| Operation | Use Case | Required Fields |
|-----------|----------|-----------------|
| `insert` | Create new record | `table`, `schema` |
| `select` | Read record | `table`, `schema` |
| `update` | Update record | `table`, `schema`, `key` |
| `delete` | Delete record | `table`, `schema`, `key` |
| `upsert` | Insert or update | `table`, `schema`, `conflict_keys` |
| `link` | Many-to-many junction | `junction`, `from_col`, `to_col` |
| `unlink` | Remove junction | `junction`, `from_col`, `to_col` |
| `list_by_fk` | List by foreign key | `table`, `schema`, `fk_col` |
| `entity_create` | Create entity + extension | `base_table`, `extension_table` |
| `entity_upsert` | Upsert entity + extension | `base_table`, `extension_table` |

### 3.4 args Field

```yaml
args:
  - name: entity-id              # REQUIRED - DSL argument name (kebab-case)
    type: uuid                   # REQUIRED - see valid types below
    required: true               # REQUIRED - is argument mandatory?
    maps_to: entity_id           # SQL column name (snake_case)
    description: "..."           # Optional description
    lookup:                      # Optional - for name→UUID resolution
      table: entities
      schema: ob-poc
      entity_type: entity        # Type hint for LSP
      search_key: name           # Column to search by
      primary_key: entity_id     # Column to return
      resolution_mode: entity    # 'entity' or 'reference'
    default: "some_value"        # Optional default value
    valid_values:                # Optional enum validation
      - ACTIVE
      - PENDING
      - CLOSED
```

**Valid `type` values:**

| Type | DSL Example | Rust Type |
|------|-------------|-----------|
| `string` | `:name "Acme"` | String |
| `uuid` | `:id "550e8400-..."` or `:id "Acme"` (with lookup) | Uuid |
| `integer` | `:count 42` | i64 |
| `decimal` | `:rate 0.025` | Decimal |
| `boolean` | `:active true` | bool |
| `date` | `:dob "1975-03-15"` | NaiveDate |
| `timestamp` | `:created-at "2025-01-01T12:00:00Z"` | DateTime |
| `uuid_array` | `:ids ["id1", "id2"]` | Vec<Uuid> |
| `string_list` | `:tags ["a", "b"]` | Vec<String> |
| `json` | `:metadata {...}` | serde_json::Value |

**Common mistakes:**
- Using `type: text` instead of `type: string`
- Using `type: int` instead of `type: integer`
- Using `type: float` instead of `type: decimal`
- Using `type: datetime` instead of `type: timestamp`

### 3.5 lookup Field

```yaml
lookup:
  table: entities                # REQUIRED - table to search
  schema: ob-poc                 # Schema (defaults to ob-poc)
  entity_type: entity            # Type hint for LSP autocomplete
  search_key: name               # Column to search by (or s-expr)
  primary_key: entity_id         # Column to return
  resolution_mode: entity        # 'entity' (search modal) or 'reference' (dropdown)
```

**When to use lookup:**
- Argument is `type: uuid` but user provides a name
- EntityGateway resolves "Acme Fund" → "550e8400-..."
- Enables LSP autocomplete in editor

### 3.6 returns Field

```yaml
returns:
  type: uuid                     # 'uuid', 'record', 'affected', 'list'
  name: cbu_id                   # Column name (for uuid type)
  capture: true                  # Allow :as @binding syntax
```

### 3.7 produces Field (Dataflow)

```yaml
produces:
  type: cbu                      # Binding type for DAG
  subtype: fund_umbrella         # Optional subtype
  resolved: false                # false=create, true=lookup
  initial_state: DRAFT           # Optional initial lifecycle state
```

**Common `type` values:** `cbu`, `entity`, `case`, `workstream`, `resource_instance`, `holding`, `movement`

### 3.8 consumes Field (Dataflow)

```yaml
consumes:
  - arg: cbu-id                  # Which argument carries the dependency
    type: cbu                    # Expected binding type
    required: true               # Is this dependency mandatory?
  - arg: entity-id
    type: entity
    required: true
```

### 3.9 lifecycle Field

```yaml
lifecycle:
  entity_arg: investor-id        # Which arg contains the entity
  requires_states:               # Valid states before execution
    - PENDING_DOCUMENTS
  transitions_to: KYC_IN_PROGRESS  # State after execution
  writes_tables:                 # For DAG ordering
    - kyc.investors
  reads_tables:
    - kyc.investors
```

---

## 4. Complete Examples

### 4.1 Simple CRUD (Insert)

```yaml
create-share-class:
  description: Create a new share class for a fund
  behavior: crud
  produces:
    type: share_class
  crud:
    operation: insert
    table: share_classes
    schema: kyc
    returning: id
  args:
    - name: cbu-id
      type: uuid
      required: true
      maps_to: cbu_id
      lookup:
        table: cbus
        schema: ob-poc
        entity_type: cbu
        search_key: name
        primary_key: cbu_id
    - name: name
      type: string
      required: true
      maps_to: name
    - name: isin
      type: string
      required: false
      maps_to: isin
    - name: currency
      type: string
      required: true
      maps_to: currency
  returns:
    type: uuid
    name: id
    capture: true
  consumes:
    - arg: cbu-id
      type: cbu
      required: true
```

### 4.2 Upsert with Conflict Keys

```yaml
ensure:
  description: Create or update investor record
  behavior: crud
  produces:
    type: investor
    resolved: false
  crud:
    operation: upsert
    table: investors
    schema: kyc
    returning: investor_id
    conflict_keys:
      - entity_id
      - owning_cbu_id
  args:
    - name: entity-id
      type: uuid
      required: true
      maps_to: entity_id
      lookup:
        table: entities
        schema: ob-poc
        entity_type: entity
        search_key: name
        primary_key: entity_id
    - name: owning-cbu-id
      type: uuid
      required: true
      maps_to: owning_cbu_id
      lookup:
        table: cbus
        schema: ob-poc
        entity_type: cbu
        search_key: name
        primary_key: cbu_id
    - name: investor-type
      type: string
      required: true
      maps_to: investor_type
      valid_values:
        - RETAIL
        - PROFESSIONAL
        - INSTITUTIONAL
        - NOMINEE
    - name: lifecycle-state
      type: string
      required: false
      maps_to: lifecycle_state
      default: "ENQUIRY"
  returns:
    type: uuid
    name: investor_id
    capture: true
  consumes:
    - arg: entity-id
      type: entity
      required: true
    - arg: owning-cbu-id
      type: cbu
      required: true
```

### 4.3 Plugin (Custom Handler)

```yaml
start-kyc:
  description: Transition investor to KYC_IN_PROGRESS and create KYC case
  behavior: plugin
  handler: InvestorStartKycOp
  produces:
    type: case
    initial_state: OPEN
  args:
    - name: investor-id
      type: uuid
      required: true
      lookup:
        table: investors
        schema: kyc
        entity_type: investor
        search_key: investor_id
        primary_key: investor_id
    - name: priority
      type: string
      required: false
      default: "NORMAL"
      valid_values:
        - LOW
        - NORMAL
        - HIGH
        - URGENT
  returns:
    type: uuid
    name: case_id
    capture: true
  lifecycle:
    entity_arg: investor-id
    requires_states:
      - PENDING_DOCUMENTS
    transitions_to: KYC_IN_PROGRESS
  consumes:
    - arg: investor-id
      type: investor
      required: true
```

### 4.4 Update with Hardcoded Values

```yaml
approve:
  description: Approve investor KYC
  behavior: crud
  crud:
    operation: update
    table: investors
    schema: kyc
    key: investor_id
    set_values:
      kyc_status: "APPROVED"
      lifecycle_state: "KYC_APPROVED"
  args:
    - name: investor-id
      type: uuid
      required: true
      maps_to: investor_id
      lookup:
        table: investors
        schema: kyc
        entity_type: investor
        search_key: investor_id
        primary_key: investor_id
    - name: risk-rating
      type: string
      required: true
      maps_to: kyc_risk_rating
      valid_values:
        - LOW
        - MEDIUM
        - HIGH
    - name: expires-at
      type: date
      required: true
      maps_to: kyc_expires_at
  returns:
    type: affected
  lifecycle:
    entity_arg: investor-id
    requires_states:
      - KYC_IN_PROGRESS
    transitions_to: KYC_APPROVED
  consumes:
    - arg: investor-id
      type: investor
      required: true
```

---

## 5. Common Errors & Fixes

### 5.1 Verb Doesn't Load (Silent Failure)

**Symptom:** Verb not found at runtime, no error at startup.

**Causes:**
1. Invalid YAML syntax (indentation, missing colons)
2. Unknown field name (typo like `behaviuor` instead of `behavior`)
3. Invalid enum value (e.g., `type: text` instead of `type: string`)
4. Missing required field (`description`, `behavior`)

**Debug:**
```bash
cargo xtask verify-verbs
# Shows parse errors for all YAML files
```

### 5.2 Wrong behavior Value

```yaml
# WRONG - not a valid behavior
behavior: custom
behavior: handler
behavior: crud-insert
behavior: CRUD

# CORRECT
behavior: crud
behavior: plugin
```

### 5.3 Wrong type Value

```yaml
# WRONG
type: text       # Use 'string'
type: int        # Use 'integer'
type: float      # Use 'decimal'
type: datetime   # Use 'timestamp'
type: array      # Use 'uuid_array' or 'string_list'

# CORRECT
type: string
type: integer
type: decimal
type: timestamp
type: uuid_array
```

### 5.4 Missing handler for Plugin

```yaml
# WRONG - plugin requires handler
approve-kyc:
  description: Approve KYC
  behavior: plugin    # ← Where's the handler?
  args: [...]

# CORRECT
approve-kyc:
  description: Approve KYC
  behavior: plugin
  handler: InvestorApproveKycOp  # ← Must match Rust struct name
  args: [...]
```

### 5.5 lookup Without Proper Fields

```yaml
# WRONG - missing required lookup fields
args:
  - name: cbu-id
    type: uuid
    lookup:
      table: cbus
      # Missing: search_key, primary_key

# CORRECT
args:
  - name: cbu-id
    type: uuid
    lookup:
      table: cbus
      schema: ob-poc
      entity_type: cbu
      search_key: name
      primary_key: cbu_id
```

### 5.6 Kebab vs Snake Case

```yaml
# DSL arguments use kebab-case
args:
  - name: investor-id      # ← Kebab-case (DSL)
    maps_to: investor_id   # ← Snake_case (SQL column)
    
# In DSL:
# (investor.approve :investor-id "...")  ← Uses kebab-case
```

---

## 6. Validation Commands

```bash
# Verify all verb YAML files parse correctly
cargo xtask verify-verbs

# List all verbs with their domains
cargo xtask list-verbs

# Show verb details
cargo xtask show-verb cbu.create

# Check verb count per domain
cargo xtask verb-stats
```

---

## 7. Rust Struct Mapping

The YAML maps to these Rust structs (in `rust/crates/dsl-core/src/config/types.rs`):

| YAML | Rust Struct |
|------|-------------|
| Top-level | `VerbsConfig` |
| `domains.{name}` | `DomainConfig` |
| `verbs.{name}` | `VerbConfig` |
| `args[n]` | `ArgConfig` |
| `crud` | `CrudConfig` |
| `lookup` | `LookupConfig` |
| `returns` | `ReturnsConfig` |
| `produces` | `VerbProduces` |
| `consumes[n]` | `VerbConsumes` |
| `lifecycle` | `VerbLifecycle` |

Any field not matching these structs exactly will cause serde to fail silently.

---

Generated: 2026-01-09
