# CBU Structure - Entity Hierarchy Model

**Document**: `cbu-entity-hierarchy-model.md`  
**Created**: 2025-12-01  
**Status**: APPROVED DESIGN  
**Context**: Clarifying CBU structure for visualization and ownership modeling

---

## Overview

The CBU represents a client relationship with BNY. Above and within that relationship is a hierarchy of legal entities connected by ownership (via share classes).

---

## The Hierarchy

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│  [Commercial Client: Blackrock Inc (US)]                                    │
│  Head Office - Contracted with BNY                                          │
│                │                                                            │
│           HOLDS X%                                                          │
│       (ManCo shares)                                                        │
│                │                                                            │
│                ▼                                                            │
│  [ManCo: Blackrock Fund Managers Ltd (IE)]                                 │
│  Regulated Manager                                                          │
│                │                                                            │
│            MANAGES                                                          │
│                │                                                            │
│       ┌────────┼────────┬────────────────┐                                 │
│       │        │        │                │                                 │
│       ▼        ▼        ▼                ▼                                 │
│   [Fund]   [Fund]    [Fund]          [Fund]                                │
│   Global   Fixed     Real            Money                                 │
│   Equity   Income    Assets          Market                                │
│       │                                                                     │
│    ISSUES                                                                   │
│       │                                                                     │
│   ┌───┴───┐                                                                │
│   ▼       ▼                                                                │
│ [Class] [Class]                                                            │
│   A       B                                                                │
│       │                                                                     │
│    HOLDS                                                                    │
│       │                                                                     │
│       ▼                                                                     │
│  [Investors]                                                               │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Entity Levels

| Level | Example | Role | Notes |
|-------|---------|------|-------|
| **Commercial Client** | Blackrock Inc | Head office, signed contracts | Top of ownership chain |
| **ManCo** | Blackrock Fund Managers Ltd | Regulated manager | May be 100% owned, JV, or mixed |
| **Fund Entity** | Blackrock Global Equity Fund LLC | Legal issuer | Issues fund share classes |
| **Share Class** | Class A EUR Institutional | Investment instrument | Held by investors |

---

## Key Insight: Ownership % Is Not Assumed

ManCo ownership varies:

| Structure | Ownership | Example |
|-----------|-----------|---------|
| 100% subsidiary | Simple | Most common |
| Joint venture | 51/49, 50/50 | China market entry |
| Partial acquisition | 80/20 | Bought majority |
| Management equity | 70/30 | Boutique, partners own stake |
| Regulatory requirement | Varies | Local ownership laws |

**We don't hardcode percentages. The share class holdings model captures actual ownership.**

---

## Share Classes at Two Levels

| Level | Category | Purpose | Holders |
|-------|----------|---------|---------|
| **Corporate** | CORPORATE | Ownership/control of company | Parent entities |
| **Fund** | FUND | Investment vehicle | Investors |

Same table, different category:

```sql
kyc.share_classes.class_category = 'CORPORATE' -- ManCo shares
kyc.share_classes.class_category = 'FUND'      -- Fund shares for investors
```

---

## Edge Types

| Edge | From | To | Meaning |
|------|------|-----|---------|
| `HOLDS` | Entity/Person | Share Class | Owns shares (with %) |
| `ISSUES` | Entity | Share Class | Legal issuer |
| `MANAGES` | ManCo Entity | Fund Entity | Regulatory management |
| `CONTROLS` | Entity/Person | Entity | Non-ownership control |
| `ROLE` | Person | Entity/CBU | Officer, signatory, etc. |

---

## Where CBU Fits

**CBU = Client Relationship Container**

```
Commercial Client ──► CLIENT_OF ──► CBU (relationship with BNY)
                                       │
                                       ├── ManCo Entity (linked)
                                       │
                                       └── Fund Entities (linked)
                                               │
                                               └── Share Classes
```

The CBU is the **account** with BNY. Entities are linked to it via `cbu_entity_roles`.

---

## Schema Changes Required

### 1. Add commercial client to CBU

```sql
ALTER TABLE "ob-poc".cbus 
ADD COLUMN commercial_client_entity_id UUID REFERENCES "ob-poc".entities(entity_id);

COMMENT ON COLUMN "ob-poc".cbus.commercial_client_entity_id IS 
'Head office entity that contracted with the bank (e.g., Blackrock Inc). 
Convenience field - actual ownership is in holdings chain.';
```

### 2. Add issuing entity to share_classes

```sql
ALTER TABLE kyc.share_classes 
ADD COLUMN entity_id UUID REFERENCES "ob-poc".entities(entity_id);

COMMENT ON COLUMN kyc.share_classes.entity_id IS 
'The legal entity that issues this share class';
```

### 3. Add class category to share_classes

```sql
ALTER TABLE kyc.share_classes 
ADD COLUMN class_category VARCHAR(20) DEFAULT 'FUND';

COMMENT ON COLUMN kyc.share_classes.class_category IS 
'CORPORATE = company ownership shares, FUND = investment fund shares';

-- Add check constraint
ALTER TABLE kyc.share_classes 
ADD CONSTRAINT chk_class_category 
CHECK (class_category IN ('CORPORATE', 'FUND'));
```

### 4. Add MANAGES relationship type

Either via `cbu_entity_roles` with role = 'MANAGES', or a new table:

```sql
-- Option A: Use existing roles table, add MANAGES role
INSERT INTO "ob-poc".roles (name, description) 
VALUES ('MANAGES', 'Management company manages fund entity');

-- Option B: New junction table (if we need more metadata)
CREATE TABLE "ob-poc".entity_management (
    management_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    manager_entity_id UUID NOT NULL REFERENCES entities(entity_id),
    managed_entity_id UUID NOT NULL REFERENCES entities(entity_id),
    management_type VARCHAR(30), -- INVESTMENT_MANAGER, AIFM, etc.
    effective_date DATE,
    end_date DATE,
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT now()
);
```

---

## Ownership Chain Walk

To find UBOs or commercial client:

```sql
-- Walk UP from Fund → ManCo → Commercial Client
WITH RECURSIVE ownership_chain AS (
    -- Start from fund entity
    SELECT 
        h.holder_entity_id as owner_id,
        sc.entity_id as owned_id,
        h.units_held / NULLIF(sc.shares_issued, 0) * 100 as pct,
        1 as depth
    FROM kyc.holdings h
    JOIN kyc.share_classes sc ON sc.id = h.share_class_id
    WHERE sc.entity_id = $fund_entity_id
    
    UNION ALL
    
    -- Walk up
    SELECT 
        h.holder_entity_id,
        sc.entity_id,
        oc.pct * (h.units_held / NULLIF(sc.shares_issued, 0)),
        oc.depth + 1
    FROM ownership_chain oc
    JOIN kyc.share_classes sc ON sc.entity_id = oc.owner_id
    JOIN kyc.holdings h ON h.share_class_id = sc.id
    WHERE oc.depth < 10
)
SELECT * FROM ownership_chain;
```

---

## Visualization Views

### View 1: Service Delivery Map

```
CBU
 └── Products
      └── Services
           └── Resources
```

**Focus**: What does BNY provide to this client?

### View 2: KYC/UBO Structure

```
Commercial Client
 └── ManCo (ownership %)
      └── Fund (manages)
           └── Share Classes
                └── Officers/UBOs (ownership %)
```

**Focus**: Who is this client? Who controls it?

---

## Data Model Summary

```
entities (all legal entities - Commercial Client, ManCo, Fund, Investors)
    │
    ├── cbus.commercial_client_entity_id ──► convenience link to head office
    │
    ├── cbu_entity_roles ──► links entities to CBU with roles
    │
    └── share_classes (entity_id = issuing entity)
            │
            ├── class_category: CORPORATE or FUND
            │
            └── holdings ──► who owns what %
```

The ownership chain is walked via holdings → share_classes → entity → holdings...

---

## Next Steps

1. Run schema migration (add columns)
2. Update `CbuGraphBuilder` to load entity hierarchy
3. Implement two visualization views
4. Add UBO chain calculation using ownership walk

---

*End of Model*
