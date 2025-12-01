# CBU Visualization - Layer Architecture Plan

**Document**: `cbu-visualization-layers-plan.md`  
**Created**: 2025-12-01  
**Status**: PLANNING  
**Context**: Following share class implementation, planning improved visualization

---

## Problem Statement

The current MVP radial layout doesn't:
- Show share classes as ownership instruments
- Distinguish voting vs economic control
- Handle density gracefully
- Tell the CBU story visually
- Differentiate relationship types

A weak UI makes people dismiss what they don't understand. The model is sophisticated - the visualization needs to make that **obvious**.

---

## Core Insight: Three Relationship Domains

The CBU is center, but three distinct relationship domains radiate from it:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│                              ┌─────────┐                                    │
│                              │   CBU   │                                    │
│                              └────┬────┘                                    │
│                                   │                                         │
│         ┌─────────────────────────┼─────────────────────────┐              │
│         │                         │                         │              │
│         ▼                         ▼                         ▼              │
│  ┌─────────────┐          ┌─────────────┐          ┌─────────────┐        │
│  │ STRUCTURE   │          │  DELIVERY   │          │  REGISTRY   │        │
│  │             │          │             │          │             │        │
│  │ Who is the  │          │ What do we  │          │ Who has     │        │
│  │ client?     │          │ provide?    │          │ invested?   │        │
│  └─────────────┘          └─────────────┘          └─────────────┘        │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Domain 1: STRUCTURE (Ownership & Control)

**Question**: Who is this client? Who owns/controls it?

| Edge Type | From | To | Meaning |
|-----------|------|-----|---------|
| OWNS | Entity/Person | Share Class | Holds shares |
| CONTROLS | Entity/Person | Entity | Non-ownership control |
| ROLE | Person | CBU | Officer/Signatory role |
| UBO | Person | CBU | Ultimate beneficial owner |

**Visual**: Hierarchy / tree / ownership chain  
**Key insight**: Share class is the **instrument** that connects owner to owned

```
                    [John Smith]
                         │
                    OWNS 60%
                    Class A (voting)
                         │
                         ▼
                    [ManCo Ltd]
                         │
              ┌──────────┴──────────┐
              │                     │
         ROLE: CIO            ROLE: AUTH_SIGNER
              │                     │
              ▼                     ▼
           [CBU] ◄─────────────────┘
```

---

## Domain 2: DELIVERY (Products & Services)

**Question**: What do we provide to this client?

| Edge Type | From | To | Meaning |
|-----------|------|-----|---------|
| SUBSCRIBED | CBU | Product | Client has this product |
| ACTIVATED | Product | Service | Service enabled |
| PROVISIONED | Service | Resource | Resource instance created |
| ROUTES_TO | Booking Rule | SSI | Settlement routing |

**Visual**: Service delivery map / capability tree  
**Key insight**: Product → Service → Resource hierarchy

```
           [CBU]
              │
         SUBSCRIBED
              │
              ▼
      [Global Custody]
              │
    ┌─────────┴─────────┐
    │                   │
ACTIVATED           ACTIVATED
    │                   │
    ▼                   ▼
[Safekeeping]     [Settlement]
    │                   │
PROVISIONED        PROVISIONED
    │                   │
    ▼                   ▼
[Account-001]     [SSI-US-EQ]
```

---

## Domain 3: REGISTRY (Investors)

**Question**: Who has invested in this fund?

| Edge Type | From | To | Meaning |
|-----------|------|-----|---------|
| ISSUED | CBU (Fund) | Share Class | Fund issues shares |
| HOLDS | Investor | Holding | Position in share class |
| MOVEMENT | Holding | Movement | Transaction history |

**Visual**: Investor registry / position summary  
**Key insight**: Scale is different - could be 10,000+ investors (retail fund)

```
           [Fund CBU]
              │
           ISSUES
              │
    ┌─────────┴─────────┐
    │                   │
    ▼                   ▼
[Class A EUR]     [Class I USD]
    │                   │
    │              ┌────┴────┐
    │              │         │
  HOLDS          HOLDS     HOLDS
    │              │         │
    ▼              ▼         ▼
[Pension A]   [Pension B] [SWF X]
 1000 units    5000 units  50000 units
```

---

## The Share Class Bridge

Share class appears in **two domains**:

| In Structure | In Registry |
|--------------|-------------|
| Ownership instrument | Investment vehicle |
| "John owns 60% of Class A" | "Pension Fund holds 1000 Class A units" |
| Determines voting/economic | Determines position value |
| Few holders (UBO relevant) | Many holders (investor registry) |

This is the **link**. The share class connects:
- **Who controls** (Structure)
- **Who invested** (Registry)

---

## Edge Type Taxonomy

| Domain | Edge Type | Visual Style | Direction |
|--------|-----------|--------------|-----------|
| **Structure** | OWNS | Solid, weighted by % | Up (to owner) |
| | CONTROLS | Dashed | Up (to controller) |
| | ROLE | Thin solid | Bidirectional |
| | UBO | Bold, highlighted | Up (to UBO) |
| **Delivery** | SUBSCRIBED | Solid | Down (to product) |
| | ACTIVATED | Solid | Down (to service) |
| | PROVISIONED | Solid | Down (to resource) |
| | ROUTES_TO | Dotted | Lateral (rule to SSI) |
| **Registry** | ISSUED | Solid | Down (to share class) |
| | HOLDS | Thin solid | Down (to holding) |
| | MOVEMENT | Dotted, temporal | From holding |

---

## Layer Architecture

Same CBU center, toggle what radiates out:

```
┌─────────────────────────────────────────────────────────────────┐
│                         LAYER TOGGLES                           │
│                                                                 │
│  [■ Structure]  [■ Delivery]  [□ Registry]  [□ Documents]      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                                                                 │
│                           [CBU]                                 │
│                              │                                  │
│         ┌────────────────────┼────────────────────┐            │
│         │                    │                    │            │
│    STRUCTURE              DELIVERY            REGISTRY          │
│    (if enabled)          (if enabled)        (if enabled)       │
│         │                    │                    │            │
│    ┌────┴────┐          ┌────┴────┐         ┌────┴────┐       │
│    │         │          │         │         │         │       │
│  [Person]  [Entity]  [Product] [Service]  [Class]  [Class]    │
│    │         │          │         │          │         │       │
│   ...       ...        ...       ...        ...       ...      │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

## Layer Definitions

| Layer | Nodes | Edges | Default |
|-------|-------|-------|---------|
| **Structure** | Entities, Persons, Share Classes (as ownership) | OWNS, CONTROLS, ROLE, UBO | ON |
| **Delivery** | Products, Services, Resources, SSIs, Rules | SUBSCRIBED, ACTIVATED, PROVISIONED, ROUTES_TO | ON |
| **Registry** | Share Classes (as investment), Holdings | ISSUED, HOLDS | OFF |
| **Documents** | Document requirements, Submitted docs | REQUIRES, SUBMITTED | OFF |
| **KYC Status** | KYC status badges on entities | (decorators, not edges) | OFF |

---

## Layer Behavior

**When layer is ON:**
- Nodes of that layer visible
- Edges of that layer visible
- Contributes to layout calculation

**When layer is OFF:**
- Nodes hidden
- Edges hidden
- Layout recalculates without them (less clutter)

**Shared nodes** (e.g., Share Class appears in Structure AND Registry):
- Visible if ANY layer using it is ON
- Visual style changes based on context

---

## Layer Combinations (Common Use Cases)

| Scenario | Layers | Purpose |
|----------|--------|---------|
| Compliance review | Structure only | UBO verification, control analysis |
| Operations | Delivery only | Service activation, routing check |
| Investor services | Registry only | Position summary, NAV impact |
| Full picture | All | Demo, executive overview |
| Onboarding status | Structure + Documents + KYC | Track onboarding progress |

---

## Visual Differentiation by Layer

| Layer | Node Color | Edge Style | Ring Position |
|-------|------------|------------|---------------|
| **Structure** | Blue tones | Solid, thickness = ownership % | Inner ring |
| **Delivery** | Green tones | Solid | Middle ring |
| **Registry** | Orange tones | Thin | Outer ring |
| **Documents** | Gray | Dashed | Attached to parent |
| **KYC Status** | Badge overlay | N/A | On node |

---

## Data Model Updates

The `CbuGraph` needs layer tagging on nodes and edges:

```rust
pub enum LayerType {
    Structure,   // Ownership, control, roles
    Delivery,    // Products, services, resources
    Registry,    // Share classes (as investment), holdings
    Documents,   // Doc requirements and submissions
    KycStatus,   // Status decorators
}

pub struct GraphNode {
    pub id: Uuid,
    pub node_type: NodeType,
    pub layer: LayerType,  // Which layer this belongs to
    pub label: String,
    // ...
}

pub struct GraphEdge {
    pub from: Uuid,
    pub to: Uuid,
    pub edge_type: EdgeType,
    pub layer: LayerType,  // Which layer this belongs to
    pub weight: Option<f32>,  // For ownership %
    // ...
}
```

---

## UI Toggle Component

```
┌─────────────────────────────────────┐
│  Layers                             │
│  ─────────────────────────────────  │
│  [■] Structure    (12 nodes)        │
│  [■] Delivery     (8 nodes)         │
│  [□] Registry     (2,450 nodes)     │
│  [□] Documents    (15 nodes)        │
│  [□] KYC Status                     │
└─────────────────────────────────────┘
```

The node count gives user a sense of what they're about to turn on. Registry being 2,450 nodes is a signal: "this will be dense."

---

## Registry Layer - Special Handling (CRITICAL)

Investor registry in a retail fund could be **massive** (10,000+ investors). Cannot render all nodes.

### Options:

| Approach | Behavior |
|----------|----------|
| **Collapsed by default** | Show share classes, but holdings as count badge: "1,245 holders" |
| **Expand on click** | Click share class → shows top N holders or filtered list |
| **Summary mode** | Aggregate stats: total AUM, holder count, top 10 |
| **Search/filter** | "Show holdings > $1M" or "Show pending KYC" |
| **Pagination** | Show 50 at a time with scroll/page |

### Recommended Approach:

1. **Layer ON** → Show share classes with summary badges
2. **Click share class** → Expand to show filtered holders
3. **Filter options**: Top N by value, pending KYC, recent activity

```
[Class A EUR]
├── 1,245 holders
├── €125M AUM
├── 12 pending KYC
└── [Expand →]
```

---

## Implementation Phases

### Phase 1: Layer Infrastructure
- Add `LayerType` enum to graph types
- Tag all nodes/edges with layer
- Update `CbuGraphBuilder` to assign layers

### Phase 2: UI Layer Toggles
- Add toggle panel to egui UI
- Filter visible nodes/edges by enabled layers
- Recalculate layout on toggle change

### Phase 3: Visual Differentiation
- Color schemes per layer
- Edge styles per edge type
- Ring positioning by layer

### Phase 4: Registry Handling
- Summary mode for large investor counts
- Click-to-expand behavior
- Filter/search within registry

### Phase 5: Polish
- Smooth transitions on layer toggle
- Node count badges
- Legend/key

---

## Open Questions

1. **Layout algorithm**: Does radial still work with layers, or need hierarchical for Structure?
2. **Animation**: Animate node appearance/disappearance on layer toggle?
3. **Persistence**: Remember layer preferences per user?
4. **Export**: Generate static SVG/PNG with current layer selection?

---

## Next Steps

1. Review this plan
2. Finalize Registry handling approach
3. Create implementation tickets
4. Build Phase 1 (layer infrastructure)

---

*End of Plan*
