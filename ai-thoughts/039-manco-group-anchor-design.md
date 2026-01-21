# Primary Governance Controller Model - Design Summary

## Problem Statement

For UBO purposes, the full group structure is mapped via shareholdings - which is definitive. However, there's a need for a **group anchor point** - the **Primary Governance Controller** - to:

1. Identify the entity that controls a CBU via board appointment rights (the canonical definition)
2. Use this controller to group CBUs into "books" (e.g., "Allianz Lux Book")
3. Trace the control chain upward to the ultimate parent (e.g., Allianz SE)

## Canonical Definition

> **Controlling share class = who appoints the board**

This is the primary signal. The MANAGEMENT_COMPANY role assignment is a fallback.

## Signal Priority (Deterministic)

| Priority | Signal | Source |
|----------|--------|--------|
| 1 | Board appointment rights via control share class | `kyc.special_rights` where `share_class_id IS NOT NULL` |
| 2 | MANAGEMENT_COMPANY role assignment | `cbu_entity_roles` |
| 3 | GLEIF IS_FUND_MANAGED_BY | `gleif_relationships` |

## Deterministic Tie-Break

When multiple entities qualify, the single winner is determined by:
1. Highest `board_seats`
2. Highest `voting_pct` (if `has_control`)
3. Highest `voting_pct` (if `has_significant_influence`)
4. Lowest `holder_entity_id` (UUID)

## Solution Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    Allianz SE (Ultimate Parent)                  │
│                         529900K9...                              │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ Controlling Shareholding (>50%)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│              AllianzGI GmbH (Primary Governance Controller)      │
│                         OJ2TIQSV...                              │
│                                                                  │
│    ┌──────────────────────────────────────────────────────┐     │
│    │           cbu_groups: "AllianzGI GmbH Book"          │     │
│    │           group_type: GOVERNANCE_BOOK                │     │
│    │           group_code: ALLIANZGI_LUX                  │     │
│    │           jurisdiction: LU                           │     │
│    └──────────────────────────────────────────────────────┘     │
└─────────────────────────────────────────────────────────────────┘
         │                   │                   │
         │ GOVERNANCE_       │ GOVERNANCE_       │ GOVERNANCE_
         │ CONTROLLER        │ CONTROLLER        │ CONTROLLER
         ▼                   ▼                   ▼
    ┌─────────┐        ┌─────────┐        ┌─────────┐
    │ CBU #1  │        │ CBU #2  │        │ CBU #n  │
    │ Fund A  │        │ Fund B  │        │ Fund n  │
    └─────────┘        └─────────┘        └─────────┘
```

## Key Tables

| Table | Purpose |
|-------|---------|
| `cbu_groups` | Governance-controller-anchored CBU collections ("books") |
| `cbu_group_members` | Links CBUs to groups with source tracking |
| `kyc.holding_control_links` | Materialized shareholding control relationships |

## Key Functions

| Function | Purpose |
|----------|---------|
| `kyc.fn_holder_control_position()` | Compute holder positions including class-level board rights |
| `kyc.fn_primary_governance_controller()` | Return single deterministic controller per issuer |
| `kyc.fn_compute_control_links()` | Materialize shareholding control relationships |
| `"ob-poc".fn_derive_cbu_groups()` | Auto-derive groups from governance controller (with fallback) |

## Class-Level Board Rights Flow

The key fix: board appointment rights attached to a share class flow to holders of that class.

```sql
-- Class-level board appointment rights
SELECT sr.share_class_id, sr.board_seats
FROM kyc.special_rights sr
WHERE sr.share_class_id IS NOT NULL
  AND sr.right_type = 'BOARD_APPOINTMENT';

-- Allocation policy: seats go to top eligible holder
-- (highest % of class, tie-break by UUID)
```

This enables the "controlling share class = who appoints the board" semantics.

## Data Linkage Strategy

### 1. Primary: Board Appointment via Control Share Class

```sql
-- fn_primary_governance_controller() ranks by:
-- 1. has_board_rights (from class-level + holder-level special_rights)
-- 2. has_control (voting ≥50%)
-- 3. has_significant_influence (voting ≥25%)
SELECT * FROM kyc.fn_primary_governance_controller(@issuer_entity_id);
```

### 2. Fallback: MANAGEMENT_COMPANY Role

```sql
-- Via cbu_entity_roles
SELECT c.cbu_id, c.name
FROM "ob-poc".cbu_entity_roles cer
JOIN "ob-poc".roles r ON r.role_id = cer.role_id
JOIN "ob-poc".cbus c ON c.cbu_id = cer.cbu_id
WHERE r.name = 'MANAGEMENT_COMPANY'
  AND cer.entity_id = @manco_entity_id;
```

### 3. Shareholding → Control Link

```sql
-- Compute from holdings with thresholds
SELECT * FROM kyc.fn_compute_control_links(NULL, CURRENT_DATE);
```

## Use Case Queries

### Q1: "Get all CBUs under the Allianz governance controller"

```dsl
(manco.group.cbus :manco-entity-id @allianzgi_lei)
```

### Q2: "Find the governance controller for this CBU"

```dsl
(manco.group.for-cbu :cbu-id @fund_cbu_id)
```

### Q3: "Who is the primary governance controller for this fund?"

```dsl
(manco.primary-controller :issuer-entity-id @fund_entity_id)
```

Returns:
```json
{
  "primary_controller_entity_id": "...",
  "governance_controller_entity_id": "...",
  "basis": "BOARD_APPOINTMENT",
  "board_seats": 2,
  "voting_pct": 55.00,
  "has_control": true
}
```

### Q4: "Who controls the controller? (trace to ultimate parent)"

```dsl
(manco.control-chain :manco-entity-id @allianzgi_lei)
```

Returns:
```
depth | entity_name      | controlled_by_name | voting_pct | is_ultimate
------+------------------+--------------------+------------+-------------
    1 | AllianzGI GmbH   | (none)             | (none)     | false
    2 | Allianz SE       | AllianzGI GmbH     | 100.00%    | true
```

## Group Types

| Type | Description |
|------|-------------|
| `GOVERNANCE_BOOK` | Derived from board appointment / control signals (primary) |
| `MANCO_BOOK` | Derived from MANAGEMENT_COMPANY role (fallback) |
| `CORPORATE_GROUP` | Corporate entity group (non-fund) |
| `INVESTMENT_MANAGER` | Grouped by IM rather than ManCo |
| `UMBRELLA_SICAV` | Sub-funds of a SICAV umbrella |
| `CUSTOM` | Manual grouping |

## Membership Sources

| Source | Description |
|--------|-------------|
| `GOVERNANCE_CONTROLLER` | Computed from board appointment / control signals |
| `MANCO_ROLE` | From cbu_entity_roles MANAGEMENT_COMPANY |
| `GLEIF_MANAGED` | From gleif_relationships IS_FUND_MANAGED_BY |
| `SHAREHOLDING` | From controlling shareholding |
| `MANUAL` | Manually assigned |

## Implementation Files

| File | Purpose |
|------|---------|
| `migrations/040_manco_group_anchor.sql` | Schema + functions |
| `ob-poc-types/src/manco_group.rs` | Rust types |
| `config/verbs/manco-group.yaml` | DSL verb definitions |

## Integration with Solar Navigation

For the solar system metaphor:
- **Galaxy Level**: Governance controller groups are orbital clusters
- **System Level**: A controller's CBUs are planets orbiting the controller sun
- **Planet Level**: CBU details with shareholding drill-down

```rust
// In nav_service.rs
pub fn cbus_for_controller(&self, controller_entity_id: Uuid) -> Vec<CbuRenderData> {
    // Query fn_get_manco_group_cbus for this controller
    // Return CBUs positioned in orbits
}
```

## Migration Execution

```bash
# Run migration
psql -d data_designer -f migrations/040_manco_group_anchor.sql

# Derive groups from governance controllers (with MANCO_ROLE fallback)
psql -d data_designer -c "SELECT * FROM \"ob-poc\".fn_derive_cbu_groups();"

# Compute control links from holdings
psql -d data_designer -c "SELECT kyc.fn_compute_control_links(NULL, CURRENT_DATE);"
```

## Next Steps

1. **Run migration** on ob-poc database
2. **Execute fn_derive_cbu_groups()** to populate groups from governance controllers
3. **Execute fn_compute_control_links()** to materialize shareholding controls
4. **Wire into ESPER** via the DSL verbs
5. **Integrate with solar navigation** for visual exploration

## ChatGPT Peer Review Integration

This design incorporates feedback from ChatGPT peer review:
- Class-level board rights now flow to holders (fixed bug)
- Primary governance controller is deterministic (single winner)
- Terminology updated from "ManCo" to "Primary Governance Controller"
- Signal priority codified: board rights > voting control > significant influence
