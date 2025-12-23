# CBU Architecture: Trading Matrix vs Product Subscriptions

## Core Principle

**The CBU is the unit of complexity.** It has two independent but linked aspects:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                   CBU                                        │
│                                                                              │
│   ┌───────────────────────────────────────────────────────────────────────┐ │
│   │                      TRADING MATRIX                                    │ │
│   │                      (What they trade)                                 │ │
│   │                                                                        │ │
│   │   Instrument Class ──► Lifecycle ──► Resource                         │ │
│   │        │                                                               │ │
│   │        ├── Markets                                                     │ │
│   │        ├── Currencies                                                  │ │
│   │        └── Counterparties                                              │ │
│   │                                                                        │ │
│   │   INDEPENDENT of products - defines trading universe                   │ │
│   └───────────────────────────────────────────────────────────────────────┘ │
│                                    │                                         │
│                                    │ Product subscription adds attributes    │
│                                    ▼                                         │
│   ┌───────────────────────────────────────────────────────────────────────┐ │
│   │                    PRODUCT SUBSCRIPTIONS                               │ │
│   │                    (How we service it)                                 │ │
│   │                                                                        │ │
│   │   Product ──► Service ──► Resource                                    │ │
│   │                                                                        │ │
│   │   OVERLAYS onto trading matrix - adds service-specific attributes     │ │
│   └───────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## The Separation

### Trading Matrix (Instrument-centric)

**What they can trade** - independent of how we service it.

```
cbu_instrument_universe
├── instrument_class_id  → EQUITY, IRS, CDS, FX_FORWARD...
├── market_id            → XLON, XETR, XNYS... (for listed)
├── currency             → USD, EUR, GBP...
├── counterparty_id      → Goldman, JPM... (for OTC)
└── trading_status       → ACTIVE, SUSPENDED, PENDING

     │
     │ instrument_class triggers
     ▼

instrument_lifecycles (M:N junction)
├── instrument_class_id  → EQUITY
├── lifecycle_id         → SETTLEMENT_DVP, CORPORATE_ACTIONS, PRICING_EOD
└── is_mandatory         → true/false

     │
     │ lifecycle triggers
     ▼

lifecycle_resource_capabilities (M:N junction)
├── lifecycle_id         → SETTLEMENT_DVP
├── resource_type_id     → SAFEKEEPING_ACCOUNT, CASH_ACCOUNT, SSI_SECURITIES
└── is_required          → true/false

     │
     │ provisioned per CBU
     ▼

cbu_lifecycle_instances
├── cbu_id
├── resource_type_id
├── market_id / currency / counterparty_id  (context)
├── provider_code
└── status
```

**The Trading Matrix is self-sufficient.** Given what a CBU trades, we can derive all required lifecycles and resources without knowing which products they subscribe to.

### Product Subscriptions (Service-centric)

**How we service it** - adds attributes based on product type.

```
cbu_product_subscriptions
├── cbu_id
├── product_id           → GLOBAL_CUSTODY, PRIME_BROKERAGE, FUND_ACCOUNTING
└── status               → ACTIVE, PENDING, TERMINATED

     │
     │ product triggers
     ▼

product_services (M:N junction)
├── product_id           → GLOBAL_CUSTODY
├── service_id           → SETTLEMENT, PRICING, CORP_ACTIONS
└── is_mandatory         → true/false

     │
     │ service triggers
     ▼

service_resource_capabilities (M:N junction)
├── service_id           → SETTLEMENT
├── resource_type_id     → SWIFT_CONNECTION, CSD_LINK
└── is_required          → true/false

     │
     │ provisioned per CBU
     ▼

cbu_resource_instances
├── cbu_id
├── resource_type_id
├── context (market/currency/etc)
├── provider_code
└── status
```

---

## The Linkage: Products Add Attributes to Matrix

Products don't **define** the trading matrix. They **augment** it.

### Same Matrix Entry, Different Product Attributes

```
CBU: "Alpha Fund"
Trading Matrix Entry: EQUITY @ XLON @ GBP

┌─────────────────────────────────────────────────────────────────────────────┐
│  BASE (from Trading Matrix)                                                  │
│  ├── Lifecycle: SETTLEMENT_DVP                                              │
│  │   └── Resources: SAFEKEEPING_ACCOUNT, CASH_ACCOUNT, SSI                  │
│  ├── Lifecycle: CORPORATE_ACTIONS                                           │
│  │   └── Resources: CA_ELECTION_PLATFORM                                    │
│  ├── Lifecycle: PRICING_EOD                                                 │
│  │   └── Resources: PRICING_FEED                                            │
│  └── Lifecycle: INCOME_PROCESSING                                           │
│      └── Resources: TAX_PROFILE                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                    +
┌─────────────────────────────────────────────────────────────────────────────┐
│  PRODUCT: Global Custody                                                     │
│  ├── Adds: Custody reporting SLAs                                           │
│  ├── Adds: SWIFT connectivity requirements                                   │
│  └── Adds: Subcustodian network access                                      │
└─────────────────────────────────────────────────────────────────────────────┘
                                    +
┌─────────────────────────────────────────────────────────────────────────────┐
│  PRODUCT: Prime Brokerage                                                    │
│  ├── Adds: Margin calculation service                                       │
│  ├── Adds: Stock loan availability                                          │
│  └── Adds: Financing rate agreements                                        │
└─────────────────────────────────────────────────────────────────────────────┘
                                    +
┌─────────────────────────────────────────────────────────────────────────────┐
│  PRODUCT: Fund Accounting                                                    │
│  ├── Adds: NAV calculation service                                          │
│  ├── Adds: NAV delivery SLAs                                                │
│  └── Adds: Pricing source priority                                          │
└─────────────────────────────────────────────────────────────────────────────┘
```

### The Matrix Entry Gets Combined Attributes

```sql
-- Effective configuration for a matrix entry
SELECT 
    u.instrument_class,
    u.market,
    u.currency,
    -- Base lifecycles from instrument
    array_agg(DISTINCT l.code) as lifecycles,
    -- Product-specific SLAs
    array_agg(DISTINCT sla.template_code) as sla_commitments,
    -- Product-specific services
    array_agg(DISTINCT s.service_code) as services
FROM cbu_instrument_universe u
JOIN instrument_lifecycles il ON il.instrument_class_id = u.instrument_class_id
JOIN lifecycles l ON l.lifecycle_id = il.lifecycle_id
-- Overlay product attributes
LEFT JOIN cbu_product_subscriptions ps ON ps.cbu_id = u.cbu_id
LEFT JOIN product_services psv ON psv.product_id = ps.product_id
LEFT JOIN services s ON s.service_id = psv.service_id
LEFT JOIN cbu_sla_commitments sla ON sla.cbu_id = u.cbu_id 
    AND sla.scope_instrument_class = u.instrument_class_id
WHERE u.cbu_id = $1
GROUP BY u.instrument_class, u.market, u.currency;
```

---

## Data Model

### Core Tables

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         REFERENCE DATA (shared)                              │
│                                                                              │
│  instrument_classes          lifecycles           lifecycle_resource_types   │
│  products                    services             service_resource_types     │
│                                                                              │
│  instrument_lifecycles (M:N)      lifecycle_resource_capabilities (M:N)     │
│  product_services (M:N)           service_resource_capabilities (M:N)       │
└─────────────────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                         CBU INSTANCE DATA                                    │
│                                                                              │
│  cbu_instrument_universe     ←── Trading Matrix (what they trade)           │
│  cbu_product_subscriptions   ←── Product Subscriptions (how we service)     │
│                                                                              │
│  cbu_lifecycle_instances     ←── Provisioned lifecycle resources            │
│  cbu_resource_instances      ←── Provisioned service resources              │
│                                                                              │
│  cbu_sla_commitments         ←── SLAs (linked to matrix + products)         │
└─────────────────────────────────────────────────────────────────────────────┘
```

### The Linkage Table

```sql
-- Links matrix entries to product-specific attributes
CREATE TABLE cbu_matrix_product_overlay (
    overlay_id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    cbu_id uuid NOT NULL REFERENCES cbus,
    
    -- Matrix context (what this overlay applies to)
    instrument_class_id uuid REFERENCES instrument_classes,
    market_id uuid REFERENCES markets,
    currency varchar(3),
    counterparty_id uuid REFERENCES entities,
    
    -- Product providing the overlay
    product_subscription_id uuid NOT NULL REFERENCES cbu_product_subscriptions,
    
    -- Attributes this product adds
    additional_services jsonb,        -- Service codes added
    additional_slas jsonb,            -- SLA template codes added
    additional_resources jsonb,       -- Resource requirements added
    product_specific_config jsonb,    -- Product-specific settings
    
    -- Status
    status varchar(20) DEFAULT 'ACTIVE',
    created_at timestamptz DEFAULT now(),
    updated_at timestamptz DEFAULT now(),
    
    -- Unique per CBU/product/context
    UNIQUE (cbu_id, product_subscription_id, instrument_class_id, market_id, currency, counterparty_id)
);
```

---

## Query Patterns

### Get Complete Matrix Entry with All Overlays

```sql
-- For a given matrix entry, get base + all product overlays
WITH matrix_entry AS (
    SELECT * FROM cbu_instrument_universe 
    WHERE cbu_id = $1 AND instrument_class_id = $2 AND market_id = $3
),
base_lifecycles AS (
    SELECT l.* FROM lifecycles l
    JOIN instrument_lifecycles il ON il.lifecycle_id = l.lifecycle_id
    WHERE il.instrument_class_id = (SELECT instrument_class_id FROM matrix_entry)
),
product_overlays AS (
    SELECT 
        p.product_code,
        o.additional_services,
        o.additional_slas,
        o.product_specific_config
    FROM cbu_matrix_product_overlay o
    JOIN cbu_product_subscriptions ps ON ps.subscription_id = o.product_subscription_id
    JOIN products p ON p.product_id = ps.product_id
    WHERE o.cbu_id = $1 
      AND o.instrument_class_id = $2 
      AND o.market_id = $3
)
SELECT 
    me.*,
    jsonb_agg(DISTINCT bl.*) as lifecycles,
    jsonb_agg(DISTINCT po.*) as product_overlays
FROM matrix_entry me
CROSS JOIN base_lifecycles bl
LEFT JOIN product_overlays po ON true
GROUP BY me.*;
```

### Gap Analysis: What's Missing for Matrix Entry + Products

```sql
-- Find resources needed but not provisioned
WITH required_resources AS (
    -- From lifecycles (base)
    SELECT DISTINCT rt.resource_type_id, rt.code, 'LIFECYCLE' as source
    FROM cbu_instrument_universe u
    JOIN instrument_lifecycles il ON il.instrument_class_id = u.instrument_class_id
    JOIN lifecycle_resource_capabilities lrc ON lrc.lifecycle_id = il.lifecycle_id
    JOIN lifecycle_resource_types rt ON rt.resource_type_id = lrc.resource_type_id
    WHERE u.cbu_id = $1 AND lrc.is_required = true
    
    UNION
    
    -- From services (product overlay)
    SELECT DISTINCT rt.resource_type_id, rt.code, 'SERVICE' as source
    FROM cbu_product_subscriptions ps
    JOIN product_services pserv ON pserv.product_id = ps.product_id
    JOIN service_resource_capabilities src ON src.service_id = pserv.service_id
    JOIN service_resource_types rt ON rt.resource_id = src.resource_id
    WHERE ps.cbu_id = $1 AND src.is_required = true
),
provisioned AS (
    SELECT resource_type_id FROM cbu_lifecycle_instances WHERE cbu_id = $1
    UNION
    SELECT resource_type_id FROM cbu_resource_instances WHERE cbu_id = $1
)
SELECT rr.* 
FROM required_resources rr
WHERE rr.resource_type_id NOT IN (SELECT resource_type_id FROM provisioned);
```

---

## Benefits of Clean Separation

### 1. Trading Matrix is Product-Agnostic

A CBU can define their trading universe without committing to products:
- "We trade European equities and IRS with Goldman"
- System discovers lifecycles and resources needed
- Products can be added/removed without changing the matrix

### 2. Products are Composable

Multiple products can overlay onto the same matrix entry:
- Global Custody + Fund Accounting on equities
- Prime Brokerage + Securities Lending on same positions
- Each adds attributes, none conflicts

### 3. Resource Sharing

Some resources serve both taxonomy domains:
- SAFEKEEPING_ACCOUNT needed by SETTLEMENT_DVP lifecycle
- SAFEKEEPING_ACCOUNT needed by CUSTODY service
- Provisioned once, linked to both

### 4. Independent Evolution

- Trading matrix can expand (new instruments, markets) without product changes
- Products can add services without trading matrix changes
- Clean boundaries, less coupling

---

## Implementation Notes

### Shared vs Separate Resource Tables

**Option A: Shared resource types (recommended)**
```
lifecycle_resource_types ←──┬──► cbu_lifecycle_instances
                            │
service_resource_types  ←───┴──► cbu_resource_instances
```
Many resources are needed by both. Share the type definitions, separate the instance tracking.

**Option B: Unified resource types**
```
resource_types ←────────────────► cbu_resource_instances
     ↑
     ├── referenced by lifecycle_resource_capabilities
     └── referenced by service_resource_capabilities
```
One resource type table, both domains reference it.

### Discovery Order

```
1. User declares trading intent (instruments, markets, counterparties)
2. System discovers required lifecycles from instrument_lifecycles
3. System discovers lifecycle resources from lifecycle_resource_capabilities
4. User subscribes to products
5. System discovers additional services from product_services
6. System discovers service resources from service_resource_capabilities
7. System merges requirements (lifecycle + service)
8. System identifies gaps
9. System provisions resources (shared where possible)
```

---

## Summary

| Aspect | Trading Matrix | Product Subscriptions |
|--------|---------------|----------------------|
| **Defines** | What they trade | How we service it |
| **Independent?** | Yes | No - overlays onto matrix |
| **Taxonomy** | Instrument → Lifecycle → Resource | Product → Service → Resource |
| **CBU Table** | `cbu_instrument_universe` | `cbu_product_subscriptions` |
| **Instance Table** | `cbu_lifecycle_instances` | `cbu_resource_instances` |
| **Gap Analysis** | What's needed to trade | What's needed to service |

**The CBU owns both. The Trading Matrix comes first. Products add attributes.**
