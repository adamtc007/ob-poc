# OB-POC Architecture — Complete System Specification

**Document Status:** Final Implementation Specification  
**Author:** Adam (Lead Solution Architect) + Claude  
**Date:** 2026-02-04  
**Version:** 3.0  
**Purpose:** Complete system architecture — Axum backend, React frontend, agent pipeline, DSL integration, decision protocol, data contracts

---

## Executive Summary

OB-POC is an **AI-assisted compliance platform** for regulated finance, not a generic chatbot. The architecture reflects this:

- **Backend:** Rust with Axum — domain model, agent orchestration, DSL engine, projection generation
- **Frontend:** React with TypeScript — pure renderer for projections and decision packets
- **Protocol:** Deterministic decision flow, not free-form chat
- **Interface:** JSON over HTTP/WebSocket, no WASM

**Key Principle:** The frontend is a **renderer**, not a decision-maker. All intent resolution, disambiguation, and execution happens server-side. The UI displays structured proposals and captures constrained responses.

---

## Table of Contents

1. [System Overview](#1-system-overview)
2. [Data Architecture](#2-data-architecture)
   - 2.1 Three-Layer Data Model
   - 2.2 RIP: Runtime Indexed Projection (Internal)
   - 2.3 Projection JSON: The Frontend Contract
   - 2.4 Paging and Scaling
   - 2.5 Taxonomy Mapping (Product/Service/Resource)
3. [Agent Pipeline](#3-agent-pipeline)
   - 3.1 Pipeline Overview
   - 3.2 Agent State Machine
   - 3.3 Tool Registry
   - 3.4 Joint Intent-Entity Scoring
4. [DSL Integration](#4-dsl-integration)
5. [Decision Protocol](#5-decision-protocol)
   - 5.1 DecisionPacket Structure
   - 5.2 Clarification Packets
   - 5.3 Proposal Packets
   - 5.4 User Response Format
   - 5.5 WebSocket Protocol
   - 5.6 Execution Gate (Mandatory)
6. [Projection API](#6-projection-api)
7. [Backend Structure](#7-backend-structure)
8. [Frontend Structure](#8-frontend-structure)
9. [API Contract](#9-api-contract)
10. [Deployment](#10-deployment)
11. [Decision Log](#11-decision-log)

**Appendices:**
- A: egui/WASM Deletion Checklist
- B: Quick Start Commands

---

## 1. System Overview

### 1.1 System Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              RUST BACKEND                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌──────────────────────────────────────────────────────────────────────┐  │
│  │                        AGENT ORCHESTRATOR                            │  │
│  │                                                                      │  │
│  │  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐   ┌──────────┐ │  │
│  │  │ Intent      │   │ Entity      │   │ Clarific-   │   │ Plan     │ │  │
│  │  │ Detection   │──▶│ Resolution  │──▶│ ation       │──▶│ Execution│ │  │
│  │  │ Service     │   │ Service     │   │ Engine      │   │ Engine   │ │  │
│  │  └─────────────┘   └─────────────┘   └─────────────┘   └──────────┘ │  │
│  │         │                 │                 │                │      │  │
│  │         ▼                 ▼                 ▼                ▼      │  │
│  │  ┌─────────────────────────────────────────────────────────────────┐│  │
│  │  │                    DECISION PACKET GENERATOR                    ││  │
│  │  │  Proposals • Clarifications • Confirmations • Results           ││  │
│  │  └─────────────────────────────────────────────────────────────────┘│  │
│  └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐ │
│  │ Domain Model    │  │ DSL Engine      │  │ Projection System           │ │
│  │                 │  │                 │  │                             │ │
│  │ • Snapshot      │  │ • Parser (nom)  │  │ • Generator                 │ │
│  │ • CBU/Entity    │  │ • Validator     │  │ • Validator                 │ │
│  │ • Product/Matrix│  │ • Executor      │  │ • Policy                    │ │
│  │ • Registers     │  │ • Completions   │  │ • Paging                    │ │
│  │                 │  │                 │  │                             │ │
│  │ (RIP internal)  │  │ (generates DSL) │  │ (generates JSON for UI)    │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘ │
│                                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐ │
│  │ GLEIF           │  │ Persistence     │  │ Tool Registry               │ │
│  │ Integration     │  │                 │  │                             │ │
│  │                 │  │ • Postgres      │  │ • entity_search             │ │
│  │ • API Client    │  │ • pgvector      │  │ • gleif_lookup              │ │
│  │ • Entity Link   │  │ • Sessions      │  │ • dsl_execute               │ │
│  │ • Hierarchy     │  │ • Snapshots     │  │ • projection_generate       │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘ │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │                         AXUM API LAYER                                  ││
│  │  HTTP: /api/projections, /api/entities, /api/dsl                        ││
│  │  WS:   /api/chat/stream (tokens + decision packets)                     ││
│  └─────────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      │ JSON / WebSocket
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           REACT FRONTEND                                    │
│                           (Pure Renderer)                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │                      CHAT / DECISION UI                                 ││
│  │                                                                         ││
│  │  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐ ││
│  │  │ Message     │   │ Clarific-   │   │ Proposal    │   │ Confirm-    │ ││
│  │  │ Stream      │   │ ation Card  │   │ Card        │   │ ation Card  │ ││
│  │  │ Renderer    │   │ (A/B/C)     │   │ (DSL view)  │   │ (execute)   │ ││
│  │  └─────────────┘   └─────────────┘   └─────────────┘   └─────────────┘ ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │                      INSPECTOR UI                                       ││
│  │                                                                         ││
│  │  ┌─────────────┐   ┌─────────────┐   ┌─────────────┐   ┌─────────────┐ ││
│  │  │ Tree        │   │ Table       │   │ Detail      │   │ Provenance  │ ││
│  │  │ Renderer    │   │ Renderer    │   │ Pane        │   │ Card        │ ││
│  │  └─────────────┘   └─────────────┘   └─────────────┘   └─────────────┘ ││
│  └─────────────────────────────────────────────────────────────────────────┘│
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │  API Client • TanStack Query • Zustand State • WebSocket Manager        ││
│  └─────────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────┘
```

### 1.2 Key Design Principles

| Principle | Implementation |
|-----------|----------------|
| **UI is a renderer** | No business logic in React; all decisions server-side |
| **Deterministic flow** | Structured decision packets, not free-form interpretation |
| **Provenance required** | Every data point traceable to source |
| **No hallucinations** | Constrained responses, explicit confirmations |
| **Compliance-first** | Audit trail, evidence chains, confidence levels |

---

## 2. Data Architecture

### 2.1 Three-Layer Data Model

```
┌─────────────────────────────────────────────────────────────────┐
│  Layer 1: DOMAIN MODEL (Rust structs)                           │
│  - CBU, Entity, Product, Matrix, Register                       │
│  - Business logic, validation, computation                      │
│  - Internal only, never serialized to frontend                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ Flattening / Indexing
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Layer 2: RIP (Runtime Indexed Projection) — INTERNAL           │
│  - Flattened arrays, adjacency lists, indices                   │
│  - Optimized for fast traversal and query                       │
│  - NEVER sent to browser                                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              │ Projection Generator
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│  Layer 3: PROJECTION JSON (Frontend contract)                   │
│  - Node map with $ref links                                     │
│  - Display fields (labels, glyphs, badges)                      │
│  - Paging metadata                                              │
│  - THIS is what React renders                                   │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 RIP: Runtime Indexed Projection (Internal)

**RIP** is an internal Rust data structure optimized for:
- Fast graph traversal
- Efficient query execution
- DSL evaluation
- Agent tool dispatch

**RIP is NOT:**
- A serialization format
- Sent to the browser
- The projection contract

#### 2.2.1 RIP Structure

```rust
// INTERNAL ONLY — never serialized to JSON
pub struct RIP {
    // === Entity Storage (SoA for cache efficiency) ===
    pub entity_ids: Vec<EntityId>,
    pub entity_index: HashMap<EntityId, usize>,
    pub entity_kinds: Vec<EntityKind>,
    pub entity_names: Vec<String>,
    pub entity_leis: Vec<Option<LEI>>,
    
    // === Adjacency Lists ===
    pub holdings_from: Vec<Vec<usize>>,  // entity → holdings edges
    pub holdings_to: Vec<Vec<usize>>,    // entity → held-by edges
    pub control_children: Vec<Vec<usize>>,
    pub control_parent: Vec<Option<usize>>,
    
    // === Pre-computed Views ===
    pub member_lists: HashMap<CbuId, Vec<usize>>,
    pub matrix_slices: HashMap<SliceKey, MatrixSlice>,
    
    // === Taxonomy (see §2.5) ===
    pub taxonomy: TaxonomyRIP,
    
    // === Provenance ===
    pub provenance: Vec<Provenance>,
}
```

#### 2.2.2 What to KEEP in RIP

| Field | Purpose |
|-------|---------|
| Stable IDs and type tags | Node identification |
| Adjacency lists | Edge traversal |
| Pre-sliced views | Fast list generation |
| Deterministic ordering keys | Consistent output |
| Provenance pointers | Compliance tracing |
| Taxonomy adjacency | Product/Service/Resource traversal |

#### 2.2.3 What to DELETE (egui/astro remnants)

| Field | Why Delete |
|-------|------------|
| Layout coordinates (x, y, z) | UI concern, not data |
| Camera anchors | 3D rendering only |
| Animation state | UI concern |
| LOD-by-distance | 3D rendering only |
| Vertex buffers | GPU rendering only |
| Draw buckets | GPU rendering only |
| Picking acceleration structures | 3D interaction only |

### 2.3 Projection JSON: The Frontend Contract

The Projection is what React receives and renders.

#### 2.3.1 Top-Level Structure

```typescript
interface InspectorProjection {
  meta: ProjectionMeta;
  render_policy: RenderPolicy;
  root: Record<string, RefValue>;
  nodes: Record<string, Node>;
}

interface ProjectionMeta {
  schema_version: number;      // Currently 9
  projection_id: string;
  snapshot_id: string;
  source_hash: string;
  created_at: string;          // ISO8601
  node_count: number;
  truncated: boolean;          // True if paging applies
}
```

#### 2.3.2 Node as Discriminated Union

TypeScript uses discriminated unions for type safety:

```typescript
// Base fields all nodes share
interface NodeBase {
  kind: NodeKind;
  id: string;                  // NodeId format: "prefix:qualifier"
  label_short: string;
  label_full?: string;
  glyph?: string;
  badges?: Badge[];
  severity?: 'info' | 'warning' | 'error';
  provenance?: Provenance;
}

// Discriminated union by kind
type Node =
  | CBUNode
  | MemberListNode
  | EntityNode
  | ProductTreeNode
  | ProductNode
  | ServiceNode
  | ResourceNode
  | ProductBindingNode
  | InstrumentMatrixNode
  | MatrixSliceNode
  | InvestorRegisterNode
  | HoldingEdgeListNode
  | HoldingEdgeNode
  | ControlRegisterNode
  | ControlTreeNode
  | ControlNodeNode
  | ControlEdgeNode
  | EntityRefListNode;

// Example: Entity node
interface EntityNode extends NodeBase {
  kind: 'Entity';
  entity_id: string;
  entity_kind: EntityKind;
  lei?: string;
  jurisdiction?: string;
  tags?: string[];
  links?: Record<string, RefValue>;
}

// Example: Holding edge
interface HoldingEdgeNode extends NodeBase {
  kind: 'HoldingEdge';
  from: RefValue;
  to: RefValue;
  metrics: HoldingMetrics;
  provenance: Provenance;      // REQUIRED for edges
}

// Example: Matrix slice
interface MatrixSliceNode extends NodeBase {
  kind: 'MatrixSlice';
  slice_key: string;
  axes: Record<string, string[]>;
  table: TableStructure;
}
```

#### 2.3.3 NodeSummary (Lightweight)

For list endpoints, use lightweight summaries instead of full nodes:

```typescript
/**
 * Lightweight node representation for list endpoints.
 * Full node details fetched via GET /nodes/:node_id if needed.
 */
interface NodeSummary {
  id: string;
  kind: NodeKind;
  label_short: string;
  glyph?: string;
  badges?: Badge[];
  severity?: 'info' | 'warning' | 'error';
  // NO: attributes, provenance, children, links, table, etc.
}
```

#### 2.3.4 Display Fields (Pre-Computed)

The projection includes pre-computed display fields so React doesn't compute:

```typescript
interface Badge {
  text: string;
  variant: 'default' | 'success' | 'warning' | 'error' | 'info';
}

// Example node with display fields
{
  "kind": "Entity",
  "id": "entity:uuid:fund_001",
  "label_short": "Allianz IE ETF SICAV",
  "label_full": "Allianz Ireland ETF SICAV plc — Global Equity Fund",
  "glyph": "🏛️",
  "badges": [
    { "text": "ETF", "variant": "info" },
    { "text": "UCITS", "variant": "success" }
  ],
  "severity": null,
  "entity_id": "fund_001",
  "entity_kind": "Fund",
  "lei": "549300ABC123DEF456",
  "jurisdiction": "IE"
}
```

#### 2.3.5 Payload Consistency Rule

> **All list endpoints** (`/children`, `/search`, paginated `ListStructure.items`) return `NodeSummary[]` or `RefValue[]`, **never full `Node[]`**. Full node data requires explicit fetch via `GET /nodes/:node_id`.

**Rationale:** Keeps payloads light, prevents over-fetching, enables virtualized rendering.

### 2.4 Paging and Scaling

#### 2.4.1 Paging Tokens (Opaque)

```typescript
interface ListStructure {
  total_count: number;
  items: RefValue[];           // Always refs, never inline nodes
  paging?: {
    next_cursor?: string;      // Opaque token, NOT a NodeId
    prev_cursor?: string;
  };
}
```

**Rule:** Paging cursors are opaque strings. The server controls pagination strategy. Clients must not parse or construct cursors.

#### 2.4.2 Payload Limits

| Scenario | Limit | Handling |
|----------|-------|----------|
| Initial projection load | 500 nodes | Truncate + `meta.truncated: true` |
| Matrix slice | 50 rows × 50 cols | Paginate sparse cells beyond |
| Holdings list | 100 items | Cursor pagination |
| Control tree | 50 levels | Depth limit + lazy load |
| Taxonomy children | 100 items | Cursor pagination |

#### 2.4.3 Compression

| Aspect | Rule |
|--------|------|
| **Format** | JSON only (no YAML over API) |
| **Compression** | gzip/brotli for responses > 1KB |
| **Max single response** | 1MB |
| **Fixtures** | YAML allowed for local dev/debug only |

### 2.5 Taxonomy Mapping (Product/Service/Resource)

#### 2.5.1 Taxonomy in RIP (Internal)

```rust
// rip/taxonomy.rs — INTERNAL ONLY

/// Taxonomy nodes stored as Structure-of-Arrays for cache efficiency
pub struct TaxonomyRIP {
    // Node identity
    pub node_ids: Vec<TaxNodeId>,
    pub node_index: HashMap<TaxNodeId, usize>,
    
    // Node data (SoA)
    pub node_kinds: Vec<TaxKind>,        // Product | Service | Resource
    pub node_labels: Vec<String>,
    pub node_codes: Vec<Option<String>>, // Product codes, service IDs
    
    // Adjacency (parent→children)
    pub children: Vec<Vec<usize>>,       // tax_children[parent_idx] = [child_idx, ...]
    
    // Reverse adjacency (child→parents) for breadcrumbs
    pub parents: Vec<Vec<usize>>,        // tax_parents[child_idx] = [parent_idx, ...]
    
    // Cross-links to entities (which entities use this resource?)
    pub entity_links: Vec<Vec<usize>>,   // resource_idx → [entity_idx, ...]
}

pub enum TaxKind {
    Product,
    Service,
    Resource,
}
```

#### 2.5.2 Stable Ordering Rules

Taxonomy children are ordered deterministically for byte-identical projection output:

| Context | Ordering Rule |
|---------|---------------|
| Product children (Services) | By `service_code` ascending, then `label_short` |
| Service children (Resources) | By `resource_code` ascending, then `label_short` |
| Cross-links (entities using resource) | By `entity_id` ascending |

**Implementation:** Use `BTreeMap` or explicit sort before emission.

#### 2.5.3 Taxonomy to Projection Mapping

```
RIP (internal)                         Projection JSON (frontend)
─────────────────────────────────────────────────────────────────
TaxonomyRIP.node_ids[i]           →    ProductNode.id / ServiceNode.id / ResourceNode.id
TaxonomyRIP.node_labels[i]        →    Node.label_short
TaxonomyRIP.children[i]           →    Node.children: RefValue[]  (NOT inline nodes)
TaxonomyRIP.entity_links[i]       →    ResourceNode.used_by: { $ref: "list:resource_users:xxx" }
```

**Critical rules:**
1. **No inline expansion** — Children are always `$ref` links, never embedded nodes
2. **List nodes for cross-links** — Entity cross-links emit a separate `EntityRefListNode`, not inline entity data
3. **Paging for large taxonomies** — If `children.len() > 100`, emit paged `ListStructure`

#### 2.5.4 Taxonomy Projection Example

```yaml
nodes:
  "product:equity":
    kind: "Product"
    id: "product:equity"
    label_short: "Equity Products"
    glyph: "📊"
    children:
      - { $ref: "service:trading" }
      - { $ref: "service:custody" }
      - { $ref: "service:reporting" }
    # Children are refs, NOT inline Service nodes

  "service:trading":
    kind: "Service"
    id: "service:trading"
    label_short: "Trading Services"
    parent: { $ref: "product:equity" }
    children:
      - { $ref: "resource:xlon_access" }
      - { $ref: "resource:xnys_access" }

  "resource:xlon_access":
    kind: "Resource"
    id: "resource:xlon_access"
    label_short: "XLON Market Access"
    parent: { $ref: "service:trading" }
    used_by: { $ref: "list:resource_users:xlon_access" }
    # Cross-link is a separate list node

  "list:resource_users:xlon_access":
    kind: "EntityRefList"
    id: "list:resource_users:xlon_access"
    label_short: "Entities using XLON Access"
    list:
      total_count: 47
      items:
        - { $ref: "entity:fund_001" }
        - { $ref: "entity:fund_002" }
        # ... up to page limit
      paging:
        next_cursor: "cursor_abc123"
```

---

## 3. Agent Pipeline

### 3.1 Pipeline Overview

The agent is NOT a chatbot. It's a **structured decision system**:

```
User Input
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│                    INTENT DETECTION SERVICE                     │
│                                                                 │
│  Input: Natural language + context                              │
│  Output: Intent + entities + confidence + ambiguities           │
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │ Semantic    │  │ DSL Pattern │  │ Confidence              │ │
│  │ Embedding   │  │ Matching    │  │ Scoring                 │ │
│  │ (BGE)       │  │             │  │                         │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼ IntentResult { intent, entities, confidence, ambiguities }
    │
┌─────────────────────────────────────────────────────────────────┐
│                    ENTITY RESOLUTION SERVICE                    │
│                                                                 │
│  Input: Entity mentions from intent                             │
│  Output: Resolved entities + candidates + ambiguities           │
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐ │
│  │ Local       │  │ GLEIF       │  │ Fuzzy                   │ │
│  │ Snapshot    │  │ Lookup      │  │ Matching                │ │
│  │ Search      │  │             │  │                         │ │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼ ResolutionResult { resolved[], candidates[], ambiguous[] }
    │
    ├─── If ambiguous ───┐
    │                    ▼
    │    ┌─────────────────────────────────────────────────────┐
    │    │              CLARIFICATION ENGINE                   │
    │    │                                                     │
    │    │  Generates DecisionPacket with:                     │
    │    │  • Options (A, B, C, ...)                           │
    │    │  • TYPE (request more specific type)                │
    │    │  • NARROW (request additional filter)               │
    │    │  • CANCEL                                           │
    │    └─────────────────────────────────────────────────────┘
    │                    │
    │    ◄───────────────┘ User selects option
    │
    ▼ Fully resolved
    │
┌─────────────────────────────────────────────────────────────────┐
│                    DSL GENERATION SERVICE                       │
│                                                                 │
│  Input: Resolved intent + entities                              │
│  Output: DSL instruction(s)                                     │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  Template Selection → Parameter Binding → Validation        ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
    │
    ▼ DSL { instructions[], dry_run_result }
    │
┌─────────────────────────────────────────────────────────────────┐
│                    PROPOSAL GENERATOR                           │
│                                                                 │
│  Generates DecisionPacket with:                                 │
│  • DSL preview (human-readable)                                 │
│  • Affected entities                                            │
│  • Side effects                                                 │
│  • CONFIRM / MODIFY / CANCEL options                            │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼ DecisionPacket (proposal)
    │
    │ User confirms (see §5.6 Execution Gate)
    ▼
┌─────────────────────────────────────────────────────────────────┐
│                    PLAN EXECUTION ENGINE                        │
│                                                                 │
│  Input: Confirmed DSL                                           │
│  Output: Execution result + new snapshot                        │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  DSL Executor → Snapshot Update → Projection Invalidation   ││
│  └─────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────┘
    │
    ▼ ExecutionResult { success, new_snapshot_id, changes[] }
```

### 3.2 Agent State Machine

```
                    ┌─────────────┐
                    │   IDLE      │
                    └──────┬──────┘
                           │ user message
                           ▼
                    ┌─────────────┐
                    │  DETECTING  │ ─── intent detection
                    └──────┬──────┘
                           │
              ┌────────────┼────────────┐
              │            │            │
              ▼            ▼            ▼
        ┌──────────┐ ┌──────────┐ ┌──────────┐
        │ RESOLVED │ │ AMBIGUOUS│ │  ERROR   │
        └────┬─────┘ └────┬─────┘ └────┬─────┘
             │            │            │
             │            ▼            │
             │     ┌─────────────┐     │
             │     │ CLARIFYING  │◄────┤
             │     └──────┬──────┘     │
             │            │ user       │
             │            │ choice     │
             │            ▼            │
             │     ┌─────────────┐     │
             │     │ RE-RESOLVE  │─────┤
             │     └──────┬──────┘     │
             │            │            │
             ◄────────────┘            │
             │                         │
             ▼                         │
      ┌─────────────┐                  │
      │ GENERATING  │ ─── DSL gen      │
      └──────┬──────┘                  │
             │                         │
             ▼                         │
      ┌─────────────┐                  │
      │  PROPOSING  │ ─── show plan    │
      └──────┬──────┘                  │
             │                         │
      ┌──────┼──────┐                  │
      │      │      │                  │
      ▼      ▼      ▼                  │
   CONFIRM MODIFY CANCEL               │
      │      │      │                  │
      │      │      └──────────────────┤
      │      │                         │
      │      └─── back to GENERATING   │
      │                                │
      ▼                                │
┌─────────────┐                        │
│ EXECUTING   │                        │
└──────┬──────┘                        │
       │                               │
       ▼                               │
┌─────────────┐                        │
│  COMPLETE   │────────────────────────┘
└─────────────┘        (back to IDLE)
```

### 3.3 Tool Registry

The agent dispatches to registered tools:

```rust
pub enum Tool {
    EntitySearch {
        query: String,
        filters: EntityFilters,
    },
    GleifLookup {
        lei: Option<String>,
        name: Option<String>,
    },
    ProjectionGenerate {
        snapshot_id: String,
        policy: RenderPolicy,
    },
    DslValidate {
        source: String,
    },
    DslExecute {
        source: String,
        snapshot_id: String,
    },
    SnapshotLoad {
        snapshot_id: String,
    },
}

pub struct ToolResult {
    pub tool: String,
    pub success: bool,
    pub data: serde_json::Value,
    pub error: Option<String>,
    pub duration_ms: u64,
}
```

### 3.4 Joint Intent-Entity Scoring

#### 3.4.1 The Problem with Verb-Only Matching

Naive intent detection matches user input to verbs independently of entity resolution:

```
User: "Add Allianz to the product"
Verb match: ADD (confidence: 0.95)
Entity match: "Allianz" → 3 candidates

Problem: Which "Allianz"? The verb "ADD" expects different entity kinds
depending on context (Fund→Product vs Manager→CBU).
```

#### 3.4.2 Joint Scoring Loop

Intent detection and entity resolution run as a **feedback loop**, not sequential steps:

```
┌─────────────────────────────────────────────────────────────────┐
│                    JOINT SCORING LOOP                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  User Input: "Add Allianz to the equity product"                │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  PHASE 1: Initial Verb Candidates                           ││
│  │                                                             ││
│  │  BGE embedding + DSL pattern match →                        ││
│  │    ADD_ENTITY_TO_PRODUCT (0.85)                             ││
│  │    ADD_ENTITY_TO_CBU (0.72)                                 ││
│  │    CREATE_ENTITY (0.45)                                     ││
│  └─────────────────────────────────────────────────────────────┘│
│       │                                                         │
│       │ Extract expected entity kinds per verb                  │
│       ▼                                                         │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  PHASE 2: Entity Resolution with Kind Constraints           ││
│  │                                                             ││
│  │  For ADD_ENTITY_TO_PRODUCT: expects Fund | Manager          ││
│  │    "Allianz" → Fund:Allianz_IE_ETF (0.92)                   ││
│  │    "equity product" → Product:equity_growth (0.88)          ││
│  │                                                             ││
│  │  For ADD_ENTITY_TO_CBU: expects Fund | Manager | Custodian  ││
│  │    "Allianz" → Manager:Allianz_Global_Inv (0.85)            ││
│  │    "equity product" → NO MATCH (CBU expected, not Product)  ││
│  └─────────────────────────────────────────────────────────────┘│
│       │                                                         │
│       │ Re-score verbs based on entity resolution success       │
│       ▼                                                         │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  PHASE 3: Joint Re-Scoring                                  ││
│  │                                                             ││
│  │  ADD_ENTITY_TO_PRODUCT:                                     ││
│  │    verb_score (0.85) × entity_score (0.92 × 0.88) = 0.69    ││
│  │    All slots filled ✓                                       ││
│  │    → FINAL SCORE: 0.69 + 0.2 (slot bonus) = 0.89            ││
│  │                                                             ││
│  │  ADD_ENTITY_TO_CBU:                                         ││
│  │    verb_score (0.72) × entity_score (0.85 × 0.0) = 0.0      ││
│  │    Missing slot: target CBU                                 ││
│  │    → FINAL SCORE: 0.0 (incomplete)                          ││
│  └─────────────────────────────────────────────────────────────┘│
│       │                                                         │
│       ▼                                                         │
│  Winner: ADD_ENTITY_TO_PRODUCT with entities resolved          │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

#### 3.4.3 Verb-Entity Kind Matrix

Each verb defines expected entity kinds for its slots:

```rust
// agent/intent/patterns.rs

pub struct VerbSlotRequirements {
    pub verb: Verb,
    pub slots: Vec<SlotRequirement>,
}

pub struct SlotRequirement {
    pub name: String,
    pub expected_kinds: Vec<EntityKind>,
    pub required: bool,
}

// Example: ADD_ENTITY_TO_PRODUCT
VerbSlotRequirements {
    verb: Verb::AddEntityToProduct,
    slots: vec![
        SlotRequirement {
            name: "entity".to_string(),
            expected_kinds: vec![EntityKind::Fund, EntityKind::InvestmentManager],
            required: true,
        },
        SlotRequirement {
            name: "product".to_string(),
            expected_kinds: vec![EntityKind::Product],
            required: true,
        },
    ],
}
```

#### 3.4.4 ResolutionEvent Logging

All resolution attempts are logged for calibration and debugging:

```rust
// agent/resolution/events.rs

#[derive(Serialize)]
pub struct ResolutionEvent {
    pub event_id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub session_id: SessionId,
    
    // Input
    pub user_input: String,
    pub input_embedding: Vec<f32>,
    
    // Verb scoring
    pub verb_candidates: Vec<VerbCandidate>,
    pub selected_verb: Option<Verb>,
    
    // Entity resolution
    pub entity_mentions: Vec<EntityMention>,
    pub resolution_attempts: Vec<ResolutionAttempt>,
    pub resolved_entities: Vec<ResolvedEntity>,
    
    // Joint scoring
    pub joint_scores: Vec<JointScore>,
    pub final_intent: Option<ResolvedIntent>,
    
    // Outcome
    pub required_clarification: bool,
    pub clarification_reason: Option<String>,
}

#[derive(Serialize)]
pub struct ResolutionAttempt {
    pub mention: String,
    pub expected_kinds: Vec<EntityKind>,
    pub candidates: Vec<EntityCandidate>,
    pub selected: Option<EntityId>,
    pub confidence: f32,
}
```

**Storage:** ResolutionEvents are persisted to `resolution_events` table for:
- Debugging failed resolutions
- Calibrating embedding models
- Identifying common ambiguity patterns
- Training data for future improvements

---

## 4. DSL Integration

### 4.1 DSL Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                         DSL PIPELINE                            │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  User/Agent Input                                               │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────┐                                                │
│  │   PARSER    │  nom combinators                               │
│  │             │  → AST                                         │
│  └──────┬──────┘                                                │
│         │                                                       │
│         ▼                                                       │
│  ┌─────────────┐                                                │
│  │  VALIDATOR  │  Check syntax, references, permissions         │
│  │             │  → ValidationResult { errors, warnings }       │
│  └──────┬──────┘                                                │
│         │                                                       │
│         ▼                                                       │
│  ┌─────────────┐                                                │
│  │  RESOLVER   │  Resolve entity refs, verify existence         │
│  │             │  → ResolvedAST                                 │
│  └──────┬──────┘                                                │
│         │                                                       │
│         ▼                                                       │
│  ┌─────────────┐                                                │
│  │  EXECUTOR   │  Apply to snapshot, record changes             │
│  │             │  → ExecutionResult { new_snapshot, changes }   │
│  └──────┬──────┘                                                │
│         │                                                       │
│         ▼                                                       │
│  ┌─────────────┐                                                │
│  │  PROJECTOR  │  Invalidate caches, regenerate projection      │
│  │             │  → New InspectorProjection                     │
│  └─────────────┘                                                │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 DSL Verb Registry

```rust
pub struct VerbRegistry {
    verbs: HashMap<String, VerbDefinition>,
}

pub struct VerbDefinition {
    pub name: String,
    pub domain: Domain,           // CBU, Entity, Product, Matrix, etc.
    pub category: VerbCategory,   // Create, Read, Update, Delete, Link
    pub parameters: Vec<Parameter>,
    pub requires_confirmation: bool,
    pub side_effects: Vec<SideEffect>,
}

// Example verbs
// CREATE ENTITY Fund "New Fund" WITH lei="..." jurisdiction="IE"
// LINK ENTITY fund_001 TO PRODUCT equity_growth AS member
// SET MATRIX CELL (fund_001, XLON, equity_growth) ENABLED=true
// COMPUTE UBO FOR fund_001 THRESHOLD 25%
```

### 4.3 DSL ↔ Agent Integration

The agent generates DSL through template selection:

```rust
pub struct DSLTemplate {
    pub intent_pattern: IntentPattern,
    pub template: String,
    pub parameter_bindings: Vec<ParameterBinding>,
}

// Example: "Add fund X to CBU Y"
// Intent: { action: "add", entity_type: "fund", target: "cbu" }
// Template: "LINK ENTITY {fund_id} TO CBU {cbu_id} AS member"
// Bindings: [
//   { param: "fund_id", source: "resolved_entity[0]" },
//   { param: "cbu_id", source: "context.current_cbu" }
// ]
```

---

## 5. Decision Protocol

### 5.1 DecisionPacket Structure

This is the core contract between backend and frontend for structured interaction:

```typescript
interface DecisionPacket {
  packet_id: string;
  packet_type: DecisionPacketType;
  timestamp: string;
  
  // Content varies by type
  content: 
    | ClarificationContent
    | ProposalContent
    | ConfirmationContent
    | ResultContent
    | ErrorContent;
}

type DecisionPacketType =
  | 'clarification'      // Ambiguous, need user choice
  | 'proposal'           // DSL generated, awaiting confirmation
  | 'confirmation'       // User confirmed, executing
  | 'result'             // Execution complete
  | 'error';             // Something failed
```

### 5.2 Clarification Packets

When the system can't resolve ambiguity:

```typescript
interface ClarificationContent {
  question: string;           // Human-readable question
  context: string;            // Why we're asking
  options: ClarificationOption[];
  allows_freeform: boolean;   // Can user type something else?
  timeout_seconds?: number;   // Auto-cancel after
}

interface ClarificationOption {
  key: string;                // 'A', 'B', 'C', 'TYPE', 'NARROW', 'CANCEL'
  label: string;              // Display text
  description?: string;       // Additional context
  entity_ref?: string;        // If option resolves to entity
  confidence?: number;        // Match confidence
}

// Example clarification
{
  "packet_type": "clarification",
  "content": {
    "question": "Which 'Allianz' entity did you mean?",
    "context": "Found 3 entities matching 'Allianz'",
    "options": [
      { "key": "A", "label": "Allianz IE ETF SICAV", "entity_ref": "entity:fund_001", "confidence": 0.92 },
      { "key": "B", "label": "Allianz Global Investors", "entity_ref": "entity:im_001", "confidence": 0.85 },
      { "key": "C", "label": "Allianz SE (Parent)", "entity_ref": "entity:corp_001", "confidence": 0.71 },
      { "key": "TYPE", "label": "Specify entity type (Fund, Manager, etc.)" },
      { "key": "NARROW", "label": "Add more details to narrow search" },
      { "key": "CANCEL", "label": "Cancel this request" }
    ],
    "allows_freeform": false
  }
}
```

### 5.3 Proposal Packets

When DSL is ready for confirmation:

```typescript
interface ProposalContent {
  proposal_id: string;
  summary: string;                    // Human-readable summary
  dsl_preview: string;                // The actual DSL (syntax highlighted)
  affected_entities: AffectedEntity[];
  side_effects: SideEffect[];
  reversible: boolean;
  options: ProposalOption[];
}

interface AffectedEntity {
  entity_ref: string;
  entity_label: string;
  change_type: 'create' | 'update' | 'delete' | 'link' | 'unlink';
}

interface SideEffect {
  description: string;
  severity: 'info' | 'warning';
}

interface ProposalOption {
  key: string;                        // 'CONFIRM', 'MODIFY', 'CANCEL'
  label: string;
  requires_reason?: boolean;          // MODIFY needs explanation
}

// Example proposal
{
  "packet_type": "proposal",
  "content": {
    "proposal_id": "prop_abc123",
    "summary": "Add 'Allianz IE ETF SICAV' to the Equity Growth product",
    "dsl_preview": "LINK ENTITY entity:fund_001 TO PRODUCT product:equity_growth AS member",
    "affected_entities": [
      { "entity_ref": "entity:fund_001", "entity_label": "Allianz IE ETF SICAV", "change_type": "link" },
      { "entity_ref": "product:equity_growth", "entity_label": "Equity Growth", "change_type": "update" }
    ],
    "side_effects": [
      { "description": "Matrix permissions will be recalculated", "severity": "info" }
    ],
    "reversible": true,
    "options": [
      { "key": "CONFIRM", "label": "Execute this change" },
      { "key": "MODIFY", "label": "Modify the request", "requires_reason": true },
      { "key": "CANCEL", "label": "Cancel" }
    ]
  }
}
```

### 5.4 User Response Format

User responses are **constrained**, not free-form:

```typescript
interface UserDecisionResponse {
  packet_id: string;                  // Which packet we're responding to
  choice: string;                     // 'A', 'B', 'CONFIRM', 'CANCEL', etc.
  reason?: string;                    // For MODIFY
  additional_context?: string;        // For NARROW/TYPE
}

// Example responses
{ "packet_id": "pkt_123", "choice": "A" }
{ "packet_id": "pkt_456", "choice": "CONFIRM" }
{ "packet_id": "pkt_789", "choice": "MODIFY", "reason": "Use the IE fund instead" }
{ "packet_id": "pkt_012", "choice": "NARROW", "additional_context": "The one in Ireland" }
```

### 5.5 WebSocket Protocol

The WebSocket carries both streaming tokens AND decision packets:

```typescript
// Server → Client messages
type ServerMessage =
  | { type: 'token'; token: string }
  | { type: 'packet'; packet: DecisionPacket }
  | { type: 'tool_start'; tool: string; input: unknown }
  | { type: 'tool_end'; tool: string; result: ToolResult }
  | { type: 'done'; session_state: SessionState }
  | { type: 'error'; error: string; recoverable: boolean };

// Client → Server messages
type ClientMessage =
  | { type: 'message'; content: string }
  | { type: 'decision'; response: UserDecisionResponse }
  | { type: 'cancel' };
```

### 5.6 Execution Gate (Mandatory)

#### 5.6.1 Confirmation Requirement

**Invariant:** No DSL execution occurs without explicit user confirmation.

```
┌─────────────────────────────────────────────────────────────────┐
│                     EXECUTION GATE                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  DSL Generated                                                  │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────────────────────────────────────────────────────┐│
│  │  PROPOSAL PACKET SENT TO CLIENT                             ││
│  │  packet_type: "proposal"                                    ││
│  │  contains: dsl_preview, affected_entities, side_effects     ││
│  └─────────────────────────────────────────────────────────────┘│
│       │                                                         │
│       │ ← USER MUST RESPOND                                     │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────┬─────────────┬─────────────┐                   │
│  │   CONFIRM   │   MODIFY    │   CANCEL    │                   │
│  └──────┬──────┴──────┬──────┴──────┬──────┘                   │
│         │             │             │                           │
│         ▼             │             │                           │
│  ┌─────────────┐      │             │                           │
│  │  EXECUTE    │      │             │                           │
│  │  DSL        │◄─────┘             │                           │
│  └─────────────┘  (after re-gen)    │                           │
│         │                           │                           │
│         │                           ▼                           │
│         │                    ┌─────────────┐                   │
│         │                    │  ABORT      │                   │
│         │                    │  No changes │                   │
│         │                    └─────────────┘                   │
│         ▼                                                       │
│  ┌─────────────┐                                                │
│  │  RESULT     │                                                │
│  │  PACKET     │                                                │
│  └─────────────┘                                                │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

#### 5.6.2 Server Enforcement

```rust
// agent/orchestrator.rs

impl AgentOrchestrator {
    pub async fn handle_decision(&mut self, response: UserDecisionResponse) -> Result<()> {
        let packet = self.pending_packets.get(&response.packet_id)
            .ok_or(AgentError::UnknownPacket)?;
        
        match (&packet.packet_type, response.choice.as_str()) {
            (DecisionPacketType::Proposal, "CONFIRM") => {
                // ONLY path to execution
                let dsl = self.pending_dsl.take()
                    .ok_or(AgentError::NoPendingDSL)?;
                self.execute_dsl(dsl).await
            }
            (DecisionPacketType::Proposal, "MODIFY") => {
                // Back to DSL generation with modification context
                self.regenerate_dsl(response.reason).await
            }
            (DecisionPacketType::Proposal, "CANCEL") => {
                // Abort, no execution
                self.abort_pending().await
            }
            _ => Err(AgentError::InvalidResponse)
        }
    }
}
```

#### 5.6.3 Executable Subset Restriction

**Invariant:** Only server-generated or validated DSL may execute.

The frontend NEVER submits arbitrary DSL for execution. Two paths exist:

| Path | Description | Validation |
|------|-------------|------------|
| **Agent-generated** | DSL created by agent from intent | Pre-validated during generation |
| **Direct DSL** | User types DSL in input | Must pass `executable_subset_validator` |

```rust
// dsl/validator.rs

/// Validates DSL is in the safe executable subset.
/// Rejects:
/// - DELETE operations on core entities
/// - Bulk operations without explicit enumeration
/// - System-level commands
/// - Unresolved entity references
pub fn validate_executable_subset(ast: &AST, context: &ExecutionContext) -> ValidationResult {
    let mut errors = vec![];
    
    for instruction in &ast.instructions {
        // No DELETE on protected entity kinds
        if instruction.verb == Verb::Delete {
            if context.is_protected(instruction.target) {
                errors.push(ValidationError::ProtectedEntity(instruction.target.clone()));
            }
        }
        
        // No wildcards in execution
        if instruction.has_wildcard() {
            errors.push(ValidationError::WildcardNotAllowed);
        }
        
        // All entity refs must resolve
        for entity_ref in instruction.entity_refs() {
            if !context.can_resolve(entity_ref) {
                errors.push(ValidationError::UnresolvedEntity(entity_ref.clone()));
            }
        }
    }
    
    ValidationResult { errors, warnings: vec![] }
}
```

---

## 6. Projection API

### 6.1 Endpoints

```
# Full projection (with size limits)
GET /api/projections/:id
    → { projection: InspectorProjection, validation: ValidationResult }

# Single node fetch (lazy loading)
GET /api/projections/:id/nodes/:node_id
    → { node: Node, adjacent_refs: RefValue[], breadcrumb: RefValue[] }

# Paginated children (returns summaries, not full nodes)
GET /api/projections/:id/nodes/:node_id/children?cursor=xxx&limit=50
    → { items: NodeSummary[], paging: PagingInfo }

# Generate new projection
POST /api/projections/generate
    body: { snapshot_id: string, policy: RenderPolicy }
    → { projection_id: string, projection: InspectorProjection }

# Validate projection
POST /api/projections/validate
    body: { projection: InspectorProjection }
    → { validation: ValidationResult }
```

### 6.2 Caching

```
┌─────────────────┐
│ React App       │
│                 │
│ TanStack Query  │◄─── Cache by projection_id + node_id
│ 5 min stale     │     Invalidate on DSL execution
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ HTTP Cache      │
│                 │
│ ETag + 304      │◄─── Server provides ETag from source_hash
│                 │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ Server Cache    │
│                 │
│ LRU by snapshot │◄─── Projection regeneration is expensive
│                 │     Cache until snapshot changes
└─────────────────┘
```

---

## 7. Backend Structure

### 7.1 Module Organization

```
rust/src/
├── main.rs                         # CLI entry point
├── lib.rs                          # Library root
│
├── domain/                         # Layer 1: Business entities
│   ├── mod.rs
│   ├── snapshot.rs                 # Aggregate root
│   ├── cbu.rs                      # CBU struct + logic
│   ├── entity.rs                   # Entity types
│   ├── product.rs                  # Product taxonomy
│   ├── matrix.rs                   # Instrument matrix
│   ├── register.rs                 # Holdings + control registers
│   └── provenance.rs               # Provenance types
│
├── rip/                            # Layer 2: Runtime indexed (internal)
│   ├── mod.rs
│   ├── builder.rs                  # Snapshot → RIP
│   ├── indices.rs                  # Index structures
│   ├── adjacency.rs                # Graph adjacency
│   ├── taxonomy.rs                 # Taxonomy adjacency (§2.5)
│   └── views.rs                    # Pre-computed views
│
├── projection/                     # Layer 3: JSON contract
│   ├── mod.rs
│   ├── model.rs                    # InspectorProjection, Node types
│   ├── generator.rs                # RIP → Projection
│   ├── validator.rs                # Validation
│   ├── policy.rs                   # RenderPolicy
│   ├── paging.rs                   # Cursor pagination
│   ├── labels.rs                   # Label generation
│   ├── glyphs.rs                   # Glyph assignment
│   ├── ordering.rs                 # Deterministic ordering
│   └── node_id.rs                  # NodeId type
│
├── agent/                          # Agent pipeline
│   ├── mod.rs
│   ├── orchestrator.rs             # Main loop + state machine
│   ├── intent/
│   │   ├── mod.rs
│   │   ├── detector.rs             # Intent detection service
│   │   ├── embeddings.rs           # BGE model integration
│   │   ├── patterns.rs             # DSL pattern matching
│   │   └── joint_scoring.rs        # Verb-entity joint scoring (§3.4)
│   ├── resolution/
│   │   ├── mod.rs
│   │   ├── entity.rs               # Entity resolution
│   │   ├── gleif.rs                # GLEIF integration
│   │   └── events.rs               # ResolutionEvent logging (§3.4.4)
│   ├── clarification/
│   │   ├── mod.rs
│   │   └── engine.rs               # Clarification generation
│   ├── tools/
│   │   ├── mod.rs
│   │   ├── registry.rs             # Tool registry
│   │   ├── entity_search.rs
│   │   ├── gleif_lookup.rs
│   │   ├── dsl_execute.rs
│   │   └── projection_generate.rs
│   └── decision/
│       ├── mod.rs
│       ├── packet.rs               # DecisionPacket types
│       ├── proposal.rs             # Proposal generation
│       └── gate.rs                 # Execution gate (§5.6)
│
├── dsl/                            # DSL engine
│   ├── mod.rs
│   ├── parser.rs                   # nom parser
│   ├── ast.rs                      # AST types
│   ├── validator.rs                # Syntax + semantic validation
│   ├── subset_validator.rs         # Executable subset (§5.6.3)
│   ├── resolver.rs                 # Entity reference resolution
│   ├── executor.rs                 # Execution engine
│   ├── verbs/
│   │   ├── mod.rs
│   │   ├── registry.rs             # Verb registry
│   │   ├── entity.rs               # Entity verbs
│   │   ├── product.rs              # Product verbs
│   │   ├── matrix.rs               # Matrix verbs
│   │   └── register.rs             # Register verbs
│   └── completions.rs              # Autocomplete
│
├── gleif/                          # GLEIF API client
│   ├── mod.rs
│   ├── client.rs
│   ├── types.rs
│   └── cache.rs
│
├── persistence/                    # Storage
│   ├── mod.rs
│   ├── postgres.rs                 # Postgres + pgvector
│   ├── snapshots.rs
│   ├── sessions.rs
│   ├── projections.rs              # Projection cache
│   └── resolution_events.rs        # Resolution event storage (§3.4.4)
│
└── api/                            # HTTP/WebSocket
    ├── mod.rs
    ├── server.rs                   # Axum app
    ├── state.rs                    # AppState
    ├── error.rs                    # Error types
    ├── routes/
    │   ├── mod.rs
    │   ├── projections.rs
    │   ├── chat.rs
    │   ├── dsl.rs
    │   └── entities.rs
    └── websocket.rs                # Chat streaming
```

### 7.2 Key Dependencies

```toml
[dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }

# Web framework
axum = "0.7"
tower = "0.4"
tower-http = { version = "0.5", features = ["cors", "trace", "compression-gzip"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Parsing
nom = "7"

# Database
sqlx = { version = "0.7", features = ["postgres", "runtime-tokio", "uuid", "chrono"] }

# Embeddings (BGE model)
candle-core = "0.4"
candle-nn = "0.4"
candle-transformers = "0.4"
tokenizers = "0.15"

# HTTP client
reqwest = { version = "0.11", features = ["json"] }

# Utilities
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

---

## 8. Frontend Structure

### 8.1 Project Organization

```
ob-poc-ui/
├── package.json
├── tsconfig.json
├── vite.config.ts
├── tailwind.config.ts
├── index.html
│
├── src/
│   ├── main.tsx
│   ├── App.tsx
│   ├── index.css
│   │
│   ├── api/                        # API client
│   │   ├── client.ts               # Base fetch + error handling
│   │   ├── projections.ts          # Projection endpoints
│   │   ├── chat.ts                 # Chat REST endpoints
│   │   ├── chatStream.ts           # WebSocket manager
│   │   ├── dsl.ts                  # DSL endpoints
│   │   └── entities.ts             # Entity endpoints
│   │
│   ├── types/                      # TypeScript types
│   │   ├── projection.ts           # Node, NodeSummary, Projection
│   │   ├── decision.ts             # DecisionPacket types
│   │   ├── chat.ts                 # Session, Message
│   │   ├── dsl.ts                  # DSL types
│   │   └── api.ts                  # Request/response envelopes
│   │
│   ├── stores/                     # Zustand state
│   │   ├── inspector.ts            # Projection navigation
│   │   ├── chat.ts                 # Chat sessions + streaming
│   │   └── preferences.ts          # Theme, settings
│   │
│   ├── hooks/
│   │   ├── useProjection.ts        # TanStack Query wrapper
│   │   ├── useNode.ts              # Single node fetch
│   │   ├── useChatStream.ts        # WebSocket hook
│   │   ├── useDecision.ts          # Decision response handling
│   │   └── useKeyboardNav.ts
│   │
│   ├── components/
│   │   ├── ui/                     # Shadcn primitives
│   │   ├── layout/
│   │   │   ├── AppShell.tsx
│   │   │   ├── Sidebar.tsx
│   │   │   └── ResizablePanels.tsx
│   │   └── common/
│   │       ├── Breadcrumbs.tsx
│   │       ├── LoadingSpinner.tsx
│   │       └── ErrorBoundary.tsx
│   │
│   ├── features/
│   │   ├── inspector/
│   │   │   ├── InspectorPage.tsx
│   │   │   ├── NavigationTree.tsx
│   │   │   ├── TreeNode.tsx
│   │   │   ├── TableRenderer.tsx
│   │   │   ├── DetailPane.tsx
│   │   │   ├── NodeCard.tsx
│   │   │   ├── ProvenanceCard.tsx
│   │   │   └── RefLink.tsx
│   │   │
│   │   ├── chat/
│   │   │   ├── ChatPage.tsx
│   │   │   ├── ChatSidebar.tsx
│   │   │   ├── MessageList.tsx
│   │   │   ├── StreamingMessage.tsx
│   │   │   ├── ChatInput.tsx
│   │   │   │
│   │   │   ├── decisions/          # Decision packet renderers
│   │   │   │   ├── DecisionRenderer.tsx
│   │   │   │   ├── ClarificationCard.tsx
│   │   │   │   ├── ProposalCard.tsx
│   │   │   │   ├── ResultCard.tsx
│   │   │   │   └── ErrorCard.tsx
│   │   │   │
│   │   │   └── ToolCallCard.tsx
│   │   │
│   │   └── settings/
│   │       └── SettingsPage.tsx
│   │
│   └── lib/
│       ├── cn.ts
│       └── formatters.ts
│
└── tests/
    ├── unit/
    └── e2e/
```

### 8.2 Decision Rendering

The frontend renders decision packets without interpretation:

```typescript
// features/chat/decisions/DecisionRenderer.tsx
function DecisionRenderer({ packet }: { packet: DecisionPacket }) {
  switch (packet.packet_type) {
    case 'clarification':
      return <ClarificationCard content={packet.content} onRespond={handleRespond} />;
    case 'proposal':
      return <ProposalCard content={packet.content} onRespond={handleRespond} />;
    case 'result':
      return <ResultCard content={packet.content} />;
    case 'error':
      return <ErrorCard content={packet.content} />;
  }
}
```

---

## 9. API Contract

### 9.1 Complete Endpoint Reference

#### Projections

```
GET  /api/projections
     → { projections: ProjectionSummary[] }

GET  /api/projections/:id
     → { projection: InspectorProjection, validation: ValidationResult }

GET  /api/projections/:id/nodes/:node_id
     → { node: Node, adjacent_refs: RefValue[], breadcrumb: RefValue[] }

GET  /api/projections/:id/nodes/:node_id/children
     ?cursor=xxx&limit=50
     → { items: NodeSummary[], paging: PagingInfo }

POST /api/projections/generate
     body: { snapshot_id, policy }
     → { projection_id, projection }

POST /api/projections/validate
     body: { projection }
     → { validation: ValidationResult }
```

#### Chat

```
GET  /api/chat/sessions
     → { sessions: SessionSummary[] }

POST /api/chat/sessions
     body: { title?, snapshot_context? }
     → { session: ChatSession }

GET  /api/chat/sessions/:id
     → { session: ChatSession }

DELETE /api/chat/sessions/:id
     → { deleted: true }

POST /api/chat/sessions/:id/messages
     body: { content }
     → { user_message, assistant_message }

POST /api/chat/sessions/:id/confirm
     body: { proposal_id, choice: 'CONFIRM' | 'MODIFY' | 'CANCEL', reason? }
     → { result: ExecutionResult } | { modified_proposal: ProposalContent }

WS   /api/chat/sessions/:id/stream
     ← ServerMessage (token | packet | tool_start | tool_end | done | error)
     → ClientMessage (message | decision | cancel)
```

#### DSL

```
POST /api/dsl/validate
     body: { source, snapshot_context? }
     → { valid, errors, warnings, ast? }

POST /api/dsl/execute
     body: { source, snapshot_id, dry_run? }
     → { success, results, snapshot_modified, new_snapshot_id? }

GET  /api/dsl/completions
     ?prefix=xxx&cursor=N&snapshot_context=yyy
     → { completions: Completion[] }
```

#### Entities

```
GET  /api/entities/:id
     → { entity: EntityDetail }

GET  /api/entities/search
     ?q=xxx&kinds=Fund,Manager&limit=20
     → { results: EntitySearchResult[] }
```

### 9.2 Response Envelope

```typescript
// All responses
interface ApiResponse<T> {
  data: T;
  meta: {
    request_id: string;
    timestamp: string;
    duration_ms: number;
  };
}

// Error responses
interface ApiError {
  error: {
    code: string;
    message: string;
    details?: unknown;
  };
  meta: {
    request_id: string;
    timestamp: string;
  };
}
```

---

## 10. Deployment

### 10.1 Development

```bash
# Terminal 1: Backend
cd ob-poc/rust
cargo run -- serve --port 3001

# Terminal 2: Frontend  
cd ob-poc-ui
npm run dev
# → http://localhost:5173
```

### 10.2 Production

```
┌─────────────────────────────────────────────────────────────────┐
│                       CDN / Load Balancer                       │
└─────────────────────────────────────────────────────────────────┘
              │                                │
    Static (/*.js, /*.css)            API (/api/*)
              │                                │
              ▼                                ▼
┌─────────────────────────┐      ┌─────────────────────────────┐
│  Static Host            │      │  API Cluster                │
│  (S3/CloudFront)        │      │                             │
│                         │      │  obpoc serve --port 80      │
│  ob-poc-ui/dist/        │      │                             │
└─────────────────────────┘      └──────────────┬──────────────┘
                                                │
                                 ┌──────────────┴──────────────┐
                                 │        PostgreSQL           │
                                 │        + pgvector           │
                                 └─────────────────────────────┘
```

### 10.3 Environment

**Backend:**
```bash
RUST_LOG=info
DATABASE_URL=postgres://user:pass@localhost/obpoc
GLEIF_API_KEY=xxx
BGE_MODEL_PATH=/models/bge-small-en-v1.5
API_PORT=3001
```

**Frontend:**
```bash
VITE_API_URL=http://localhost:3001
```

---

## 11. Decision Log

### ADR-001: Three-Layer Data Model

**Decision:** Separate Domain → RIP → Projection layers.

**Rationale:** Clear boundaries prevent UI concerns leaking into domain logic.

### ADR-002: RIP is Internal Only

**Decision:** RIP never serializes to the frontend.

**Rationale:** RIP contains indices and adjacency lists meaningless to the browser. Projection provides display-ready data.

### ADR-003: Discriminated Union for Nodes

**Decision:** TypeScript Node type is a discriminated union by `kind`.

**Rationale:** Strong typing, exhaustive switch statements, matches Rust enum pattern.

### ADR-004: Decision Packets for Structured Interaction

**Decision:** Agent returns DecisionPacket objects, not free-form text.

**Rationale:** Deterministic, auditable flow. UI doesn't interpret—just renders. No hallucination risk.

### ADR-005: Opaque Paging Cursors

**Decision:** Paging uses opaque cursor tokens, not NodeIds.

**Rationale:** Server controls pagination strategy. Can change implementation without breaking clients.

### ADR-006: Delete All egui/WASM Code

**Decision:** Remove egui, WASM, and all related code.

**Rationale:** Refactoring WASM/egui has proven error-prone. Fresh implementation is cleaner if 60fps needed later.

**Removed:** `rust/src/ui/`, `rust/src/astronomy/`, WASM build config, egui/eframe dependencies.

### ADR-007: JSON Only Over API

**Decision:** API serves JSON only. YAML is for fixtures/debug.

**Rationale:** JSON is native to JavaScript. Better compression. Faster parsing.

### ADR-008: NodeSummary for Lists

**Decision:** List endpoints return `NodeSummary[]`, not full `Node[]`.

**Rationale:** Keeps payloads light, prevents over-fetching, enables virtualized rendering.

### ADR-009: Mandatory Execution Gate

**Decision:** DSL execution requires explicit CONFIRM response to proposal packet.

**Rationale:** No hallucinations in regulated finance. Full audit trail. User always in control.

### ADR-010: Joint Intent-Entity Scoring

**Decision:** Intent detection and entity resolution run as feedback loop, not sequentially.

**Rationale:** Verb-only matching fails when entities constrain valid verbs. Joint scoring selects best (verb, entities) tuple.

---

## Appendix A: egui/WASM Deletion Checklist

Before removing `rust/src/ui/` and `rust/src/astronomy/`:

### A.1 Extract to `rip/` module

| Source File | Extract To | What |
|-------------|------------|------|
| `ui/flattening.rs` | `rip/builder.rs` | Snapshot → RIP transformation |
| `ui/adjacency.rs` | `rip/adjacency.rs` | Graph adjacency construction |
| `ui/indices.rs` | `rip/indices.rs` | Index building utilities |

### A.2 Extract to `projection/` module

| Source File | Extract To | What |
|-------------|------------|------|
| `ui/labels.rs` | `projection/labels.rs` | Label generation policies |
| `ui/ordering.rs` | `projection/ordering.rs` | Deterministic ordering rules |
| `ui/glyphs.rs` | `projection/glyphs.rs` | Glyph assignment by kind |

### A.3 Verify no dependencies remain

```bash
cargo build 2>&1 | grep -E "ui::|astronomy::"
# Should return nothing
```

### A.4 Safe to delete

```bash
rm -rf rust/src/ui/
rm -rf rust/src/astronomy/

# Remove from Cargo.toml: egui, eframe, egui_extras, wasm-bindgen
```

### A.5 Verify build

```bash
cargo build --release
cargo test
```

---

## Appendix B: Quick Start Commands

```bash
# Clone
git clone <repo>
cd ob-poc

# Backend
cd rust
cargo build
cargo run -- serve --port 3001

# Frontend (new terminal)
cd ob-poc-ui
npm install
npm run dev

# Open
open http://localhost:5173
```

---

## Document References

| Document | Purpose |
|----------|---------|
| `INSPECTOR_FIRST_VISUALIZATION_SPEC_v3.md` | Projection JSON schema (detailed) |
| `OBPOC_FRONTEND_REACT_TODO.md` | React implementation phases |
| This document | Complete system architecture |

---

*End of architecture specification.*
