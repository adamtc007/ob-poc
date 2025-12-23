# Instrument Lifecycle Taxonomy Architecture

## Overview

The instrument lifecycle taxonomy follows the **same pattern** as the existing Product → Service → Service Resource taxonomy:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    EXISTING PATTERN                                          │
│                                                                              │
│   Product ──(M:N)──► Service ──(M:N)──► Service Resource Type               │
│      │                  │                        │                          │
│  products         services              service_resource_types              │
│      │                  │                        │                          │
│      └──product_services──┘                      │                          │
│                         └──service_resource_capabilities──┘                  │
│                                                  │                          │
│                                    cbu_resource_instances                   │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                    NEW INSTRUMENT PATTERN                                    │
│                                                                              │
│   Instrument Class ──(M:N)──► Lifecycle ──(M:N)──► Lifecycle Resource Type  │
│         │                        │                         │                │
│   instrument_classes       lifecycles          lifecycle_resource_types     │
│         │                        │                         │                │
│         └──instrument_lifecycles─┘                         │                │
│                              └──lifecycle_resource_capabilities─┘            │
│                                                            │                │
│                                          cbu_lifecycle_instances            │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Table Mapping

| Product Pattern | Instrument Pattern | Purpose |
|-----------------|-------------------|---------|
| `products` | `instrument_classes` (custody schema) | **What** is being traded |
| `services` | `lifecycles` | **Operations** required |
| `service_resource_types` | `lifecycle_resource_types` | **Resources** operations need |
| `product_services` | `instrument_lifecycles` | Junction: what ops for what instrument |
| `service_resource_capabilities` | `lifecycle_resource_capabilities` | Junction: what resources for what op |
| `cbu_resource_instances` | `cbu_lifecycle_instances` | Provisioned resources per CBU |

---

## Data Flow

### 1. User Declares Trading Intent

```
USER: "We trade European equities and vanilla IRS with Goldman"
```

### 2. Agent Expands Scope (instrument_hierarchy.yaml)

```yaml
# "European equities" expands to:
markets: [XLON, XETR, XPAR, XAMS, XBRU]
instrument_class: EQUITY

# "vanilla IRS" expands to:
instrument_class: IRS
counterparty: Goldman Sachs
```

### 3. System Looks Up Lifecycles (instrument_lifecycles table)

```sql
SELECT l.* FROM lifecycles l
JOIN instrument_lifecycles il ON il.lifecycle_id = l.lifecycle_id
WHERE il.instrument_class_id = (SELECT class_id FROM instrument_classes WHERE code = 'EQUITY')
  AND il.is_mandatory = true;

-- Returns: SETTLEMENT_DVP, CORPORATE_ACTIONS, INCOME_PROCESSING, PRICING_EOD
```

### 4. System Looks Up Resources Per Lifecycle (lifecycle_resource_capabilities table)

```sql
SELECT lrt.* FROM lifecycle_resource_types lrt
JOIN lifecycle_resource_capabilities lrc ON lrc.resource_type_id = lrt.resource_type_id
WHERE lrc.lifecycle_id = (SELECT lifecycle_id FROM lifecycles WHERE code = 'SETTLEMENT_DVP')
  AND lrc.is_required = true;

-- Returns: SAFEKEEPING_ACCOUNT, CASH_ACCOUNT, SSI_SECURITIES, SSI_CASH
```

### 5. System Checks Provisioning (cbu_lifecycle_instances table)

```sql
SELECT * FROM "ob-poc".v_cbu_lifecycle_gaps
WHERE cbu_id = $1;

-- Returns missing resources grouped by instrument/market/counterparty
```

### 6. Agent Prompts for Missing Resources

```
AGENT: For European equities, I need to know:
       → Which custodian holds your European securities?
       
       For IRS with Goldman:
       → Do you have an ISDA Master Agreement with them?
       → What are your CSA terms?
       → How do you confirm trades? (MarkitWire, DTCC, etc.)
```

### 7. System Generates Provisioning DSL

```dsl
lifecycle.provision cbu-id:$cbu resource-type:SAFEKEEPING_ACCOUNT 
    market:XLON provider:BNYM provider-bic:IABORICUS33

lifecycle.provision cbu-id:$cbu resource-type:ISDA_AGREEMENT 
    counterparty:$goldman

lifecycle.provision cbu-id:$cbu resource-type:CSA_VM 
    counterparty:$goldman depends-on:["cbu:$cbu/lifecycle/ISDA_AGREEMENT/goldman"]

lifecycle.provision cbu-id:$cbu resource-type:CONFIRMATION_PLATFORM 
    provider:MARKITWIRE counterparty:$goldman
```

---

## Schema Details

### lifecycles Table

```sql
CREATE TABLE "ob-poc".lifecycles (
    lifecycle_id uuid PRIMARY KEY,
    code varchar(50) UNIQUE NOT NULL,     -- SETTLEMENT_DVP, CONFIRMATION, etc.
    name varchar(255) NOT NULL,
    description text,
    category varchar(100) NOT NULL,       -- SETTLEMENT, OTC_LIFECYCLE, COLLATERAL, etc.
    owner varchar(100) NOT NULL,          -- CUSTODY, DERIVATIVES, etc.
    regulatory_driver varchar(100),       -- UMR, EMIR, FATCA_CRS, etc.
    sla_definition jsonb,
    is_active boolean DEFAULT true
);
```

### lifecycle_resource_types Table

```sql
CREATE TABLE "ob-poc".lifecycle_resource_types (
    resource_type_id uuid PRIMARY KEY,
    code varchar(50) UNIQUE NOT NULL,     -- SAFEKEEPING_ACCOUNT, ISDA_AGREEMENT, etc.
    name varchar(255) NOT NULL,
    resource_type varchar(100) NOT NULL,  -- ACCOUNT, AGREEMENT, CONNECTIVITY, etc.
    owner varchar(100) NOT NULL,
    location_type varchar(100),           -- CSD_OR_CUSTODIAN, TRI_PARTY_AGENT, etc.
    per_currency boolean DEFAULT false,
    per_counterparty boolean DEFAULT false,
    per_market boolean DEFAULT false,
    vendor_options jsonb,                 -- Valid providers
    provisioning_verb varchar(100),       -- DSL verb to create this resource
    provisioning_args jsonb,              -- Default args
    depends_on jsonb,                     -- Resource dependencies
    is_active boolean DEFAULT true
);
```

### instrument_lifecycles Junction Table

```sql
CREATE TABLE "ob-poc".instrument_lifecycles (
    instrument_lifecycle_id uuid PRIMARY KEY,
    instrument_class_id uuid NOT NULL,    -- FK to custody.instrument_classes
    lifecycle_id uuid NOT NULL,           -- FK to lifecycles
    is_mandatory boolean DEFAULT true,
    requires_isda boolean DEFAULT false,
    display_order integer DEFAULT 100,
    UNIQUE (instrument_class_id, lifecycle_id)
);
```

### lifecycle_resource_capabilities Junction Table

```sql
CREATE TABLE "ob-poc".lifecycle_resource_capabilities (
    capability_id uuid PRIMARY KEY,
    lifecycle_id uuid NOT NULL,
    resource_type_id uuid NOT NULL,
    is_required boolean DEFAULT true,
    priority integer DEFAULT 100,         -- For fallback ordering
    UNIQUE (lifecycle_id, resource_type_id)
);
```

### cbu_lifecycle_instances Table

```sql
CREATE TABLE "ob-poc".cbu_lifecycle_instances (
    instance_id uuid PRIMARY KEY,
    cbu_id uuid NOT NULL,
    resource_type_id uuid NOT NULL,
    instance_identifier varchar(255),     -- "XLON-Securities-BNY"
    instance_url varchar(500) UNIQUE,     -- For dependency tracking
    -- Context scoping
    market_id uuid,                       -- If per_market
    currency varchar(3),                  -- If per_currency
    counterparty_entity_id uuid,          -- If per_counterparty
    -- Provider
    provider_code varchar(50),
    provider_account varchar(100),
    provider_bic varchar(11),
    -- Status
    status varchar(50) DEFAULT 'PENDING',
    -- Lifecycle timestamps
    provisioned_at timestamptz,
    activated_at timestamptz,
    ...
);
```

---

## DSL Verb Mapping

| Verb | Purpose |
|------|---------|
| `lifecycle.read` | Read lifecycle definition |
| `lifecycle.list` | List lifecycles by category/owner |
| `lifecycle.list-by-instrument` | Get lifecycles for instrument class |
| `lifecycle.list-resources-for-lifecycle` | Get resources required by lifecycle |
| `lifecycle.provision` | Provision a lifecycle resource instance |
| `lifecycle.activate` | Activate provisioned instance |
| `lifecycle.analyze-gaps` | Find missing resources for CBU |
| `lifecycle.check-readiness` | Check if CBU can trade instrument |
| `lifecycle.generate-plan` | Generate provisioning DSL |
| `lifecycle.execute-plan` | Execute provisioning DSL |
| `lifecycle.discover` | Full discovery for instrument type |
| `lifecycle.visualize-coverage` | Coverage map for CBU |

---

## Gap Analysis Views

### v_cbu_lifecycle_coverage

Shows lifecycle coverage status for each CBU universe entry:

```
cbu_id | instrument_class | market | lifecycle_code | required_resources | provisioned_resources | is_fully_provisioned
-------|------------------|--------|----------------|--------------------|-----------------------|---------------------
abc123 | EQUITY           | XLON   | SETTLEMENT_DVP | 4                  | 4                     | true
abc123 | EQUITY           | XLON   | CORPORATE_ACTIONS | 2               | 2                     | true
abc123 | IRS              | (null) | CONFIRMATION   | 1                  | 0                     | false
abc123 | IRS              | (null) | COLLATERAL_VM  | 3                  | 0                     | false
```

### v_cbu_lifecycle_gaps

Shows missing resources for CBU universe entries:

```
cbu_name | instrument_class | counterparty | lifecycle_code | missing_resource | provisioning_verb
---------|------------------|--------------|----------------|------------------|------------------
Fund A   | IRS              | Goldman      | CONFIRMATION   | CONFIRMATION_PLATFORM | service-resource.provision
Fund A   | IRS              | Goldman      | COLLATERAL_VM  | ISDA_AGREEMENT   | isda.create
Fund A   | IRS              | Goldman      | COLLATERAL_VM  | CSA_VM           | isda.add-csa
```

---

## Benefits of This Architecture

1. **Reuses proven pattern** - Same structure as product/service/resource
2. **Normalised data** - Junction tables, not denormalised arrays
3. **Queryable gaps** - SQL views for gap analysis
4. **Extensible** - Add new lifecycles/resources via YAML seed, not code
5. **Location-aware** - Resources know if they're per-market, per-currency, per-counterparty
6. **Dependency tracking** - Resources know their dependencies for ordered provisioning
7. **Provisioning verbs** - Each resource type knows which DSL verb creates it
8. **Audit trail** - Instance table tracks full lifecycle with timestamps

---

## Files

| File | Purpose |
|------|---------|
| `config/ontology/instrument_lifecycle_taxonomy.yaml` | Lifecycle and resource type definitions (seed data) |
| `config/verbs/lifecycle.yaml` | DSL verbs for lifecycle operations |
| `config/agent/lifecycle_intents.yaml` | Agent intents for lifecycle discovery |
| `migrations/202412_instrument_lifecycle_taxonomy.sql` | Schema migration |
