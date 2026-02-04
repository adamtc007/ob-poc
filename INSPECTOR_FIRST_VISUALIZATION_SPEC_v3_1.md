# Inspector-First Visualization — Technical Specification v3

**Document Status:** Pre-implementation review draft (revised)  
**Author:** Adam (Lead Solution Architect) + Claude (Architecture Review) + ChatGPT (Peer Review)  
**Date:** 2026-02-04  
**Purpose:** Comprehensive specification for implementation — incorporates peer review feedback

---

## Executive Summary

OB-POC requires visualization of complex nested structures: CBU containers, Instrument Matrices, Investor/UBO registers, and Product taxonomies. The original "Astronomy/Esper" 3D visualization approach proved high-risk due to LOD/animation/picking tuning costs and poor explainability for compliance use cases.

This document specifies an **Inspector-First** approach: a deterministic YAML/JSON projection system with a tree/table UI, stable `$ref` linking, paging, filtering, and navigation semantics. The projection contract is designed to be audit-friendly, serializable, diffable, and extensible to other rendering backends (including a future return to spatial visualization).

**Key Design Principles:**
1. **Correctness over polish** — Every visualization must be explainable and traceable
2. **Determinism** — Same input + policy = same output, always (see §3.4 for ordering rules)
3. **Separation of concerns** — Projection contract is independent of renderer
4. **Compliance-first** — Provenance, confidence, and audit trails are first-class

---

## Table of Contents

1. [Problem Analysis](#1-problem-analysis)
2. [Architecture Overview](#2-architecture-overview)
3. [Projection Schema Specification](#3-projection-schema-specification)
4. [Node Model Specification](#4-node-model-specification)
5. [Reference System Specification](#5-reference-system-specification)
6. [Render Policy Specification](#6-render-policy-specification)
7. [Navigation Semantics](#7-navigation-semantics)
8. [Domain-Specific Patterns](#8-domain-specific-patterns)
9. [Validation Requirements](#9-validation-requirements)
10. [UI Shell Specification](#10-ui-shell-specification)
11. [Implementation Plan](#11-implementation-plan)
12. [Risk Analysis](#12-risk-analysis)
13. [Open Questions](#13-open-questions)
14. [Appendices](#14-appendices)

---

## 1. Problem Analysis

### 1.1 Why Astronomy UI Failed

| Issue | Impact | Root Cause |
|-------|--------|------------|
| LOD tuning complexity | Weeks of iteration for marginal UX gains | Retained-mode 3D in egui lacks mature tooling |
| Picking precision | Click targets unreliable at high node density | Z-fighting, projection math edge cases |
| Animation state management | Hard to pause/inspect mid-transition | Stateful animation interleaved with data updates |
| Explainability gap | Cannot answer "why is X connected to Y?" | Visual position ≠ semantic relationship |
| Performance cliffs | Frame drops at ~500 visible nodes | No virtualization strategy for dense graphs |

**Conclusion:** The Astronomy approach optimizes for *impression* over *comprehension*. In regulated finance, comprehension wins.

### 1.2 What We Actually Need

| Requirement | Rationale |
|-------------|-----------|
| **Faithful representation** | Every struct field must be inspectable; no lossy summarization |
| **Stable identifiers** | Node IDs must be deterministic and greppable across sessions |
| **Provenance visibility** | Every edge/assertion must show sources, dates, confidence |
| **Selective expansion** | Users must control what's visible without payload explosion |
| **Navigation history** | Back/forward, breadcrumbs, pinned nodes for complex exploration |
| **Diffability** | Compare snapshots to see what changed (compliance audit trail) |
| **Search** | Find nodes by ID, entity_id, label text, or attribute value |

### 1.3 Constraints

- **Rust/egui stack** — Must integrate with existing OB-POC codebase
- **WASM-compatible** — Eventual browser deployment (no native-only dependencies)
- **Offline-capable** — Projections are static artifacts; no live server required
- **Schema evolution** — Must handle version skew between projection and viewer

---

## 2. Architecture Overview

### 2.1 Conceptual Model

```
┌─────────────────────────────────────────────────────────────────┐
│                     OB-POC Snapshot Model                       │
│  (CBU, Entities, Products, Matrix, Registers — Rust structs)    │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Projection Generator                         │
│  - Walks snapshot graph                                         │
│  - Applies RenderPolicy (filters, LOD, depth limits)            │
│  - Emits InspectorProjection (YAML/JSON)                        │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                   InspectorProjection                           │
│  - Envelope (snapshot metadata, policy, ui_hints)               │
│  - Root refs (entry points into node graph)                     │
│  - Nodes map (Map<NodeId, Node>)                                │
│  - All node-to-node links are $ref strings                      │
└─────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
┌─────────────────────────────┐     ┌─────────────────────────────┐
│    Inspector UI             │     │    Astronomy UI             │
│  (tree + table + detail)    │     │  (3D spatial — future)      │
└─────────────────────────────┘     └─────────────────────────────┘
```

### 2.2 Key Architectural Decisions

| Decision | Rationale | Trade-off |
|----------|-----------|-----------|
| **Projection is a separate artifact** | Decouples snapshot schema from UI concerns; enables caching, diffing, versioning | Extra serialization step; potential staleness |
| **All node-to-node links are `$ref` strings** | Flat node map enables random access; no recursive descent required | Requires validator to ensure referential integrity |
| **Paging via `$ref` to next page node** | Simple, stateless; no cursor/offset machinery (v1); future versions may use opaque tokens | Page nodes are "real" nodes in the map (slight redundancy) |
| **RenderPolicy is declarative** | Same policy + same snapshot = same projection (determinism) | Complex policies may need iteration to get right |
| **LOD is per-policy, not per-node** | Simpler generator; consistent verbosity across projection | Cannot have mixed LOD in single projection (use multiple projections if needed) |

### 2.3 Inline Value Objects vs Node References

**Critical Clarification:** The rule "all links are `$ref` strings" applies to **node-to-node relationships** only.

**Allowed inline (value objects):**
- `list.paging` — pagination metadata
- `table` — row/column data for matrix slices
- `metrics` — numeric measurements on edges
- `axes` — matrix dimension definitions
- `provenance` — source/confidence metadata
- `summary` — aggregated counts/stats
- `attributes` — key-value properties

**Must be `$ref` (domain nodes):**
- Entity references (`from`, `to`, `entity`)
- Branch targets (`branches.members`, `branches.products`)
- List items that are nodes (`list.items[*]`)
- Cross-chamber links (`links.matrix`, `links.holdings`)

**Rule of thumb:** If it has an `id` and `kind`, it's a node and must be referenced via `$ref`. If it's a nested data structure without identity, it can be inline.

### 2.4 Module Layout

```
rust/src/ui/inspector/
├── mod.rs              # Public API, re-exports
├── model.rs            # InspectorProjection, Node, RenderPolicy structs + serde
├── node_id.rs          # NodeId newtype, parsing, validation
├── ref_value.rs        # RefValue ($ref) newtype, resolution
├── validate.rs         # Referential integrity, cycle detection, schema validation
├── generator.rs        # Projection generator from snapshot (Phase 1)
├── policy.rs           # RenderPolicy application logic
├── render/
│   ├── mod.rs          # Renderer trait, registry
│   ├── tree.rs         # Tree/outline renderer
│   ├── table.rs        # Table/matrix renderer
│   ├── detail.rs       # Detail pane renderer
│   └── provenance.rs   # Provenance/evidence sub-renderer
├── navigation.rs       # Focus stack, breadcrumbs, history, pinning
├── search.rs           # Node search by ID, entity_id, label, attributes
├── labels.rs           # Shorthand label generation rules
└── diff.rs             # Projection diff (Phase 5)
```

---

## 3. Projection Schema Specification

### 3.1 Top-Level Envelope

```yaml
# InspectorProjection envelope
snapshot:
  schema_version: <integer>           # Required. Projection schema version (current: 9)
  source_hash: <string>               # Required. Hash/ID of source snapshot
  policy_hash: <string>               # Required. Hash of RenderPolicy used
  created_at: <ISO8601 datetime>      # Required. Generation timestamp
  chambers: [<string>, ...]           # Required. List of included chambers

render_policy:
  # See Section 6 for full specification
  lod: <0|1|2|3>
  max_depth: <integer>
  max_items_per_list: <integer>
  show: { ... }
  prune: { ... }

ui_hints:
  shorthand_labels: <boolean>         # Use abbreviated labels
  link_style: <"ref"|"inline">        # How to render $ref (always "ref" for v1)
  breadcrumb: <boolean>               # Show breadcrumb trail
  history: <boolean>                  # Enable back/forward navigation

root:
  <chamber_name>: { $ref: "<NodeId>" }
  # One entry per chamber; these are the navigation entry points

nodes:
  "<NodeId>": { <Node> }
  # Flat map of all nodes
```

### 3.2 Schema Version Contract

| Version | Changes |
|---------|---------|
| 9 | Initial stable schema (this document) |

**Compatibility Rule:** Viewers MUST reject projections with `schema_version` greater than their supported maximum. Viewers SHOULD attempt to render projections with lower versions but MAY warn about missing features.

### 3.3 Chamber Enumeration

Valid chamber names (extensible):

| Chamber | Description |
|---------|-------------|
| `cbu` | Client Business Unit container |
| `products` | Product → Service → Resource taxonomy |
| `matrix` | Instrument capability matrix |
| `registers` | Investor registers (economic + control/UBO) |

### 3.4 Deterministic Ordering Rules

To ensure identical input + policy produces identical output (byte-for-byte reproducible):

| Element | Ordering Rule |
|---------|---------------|
| `nodes` map keys | Lexicographic by NodeId string |
| `root` map keys | Lexicographic by chamber name |
| `list.items` arrays | Preserve source order; if generated, sort by item's `id` |
| `branches` map keys | Lexicographic by branch name |
| `children` arrays | Preserve source order; if generated, sort by child's `id` |
| `table.rows` | Preserve source order; if generated, sort by first column value |
| `table.columns` | Preserve source order (semantic ordering, not alphabetic) |
| `sparse_cells.items` | Sort by key tuple lexicographically |
| `tags` arrays | Lexicographic |
| `provenance.sources` | Lexicographic |

**Implementation Note:** Use `BTreeMap` instead of `HashMap` in Rust for deterministic iteration. When serializing, ensure YAML/JSON emitters preserve key order.

### 3.5 LOD and Field Emission Strategy

**v1 Rule (current):** Generator ALWAYS emits all fields for all nodes. The renderer applies LOD to control *display*, not *presence*.

**Rationale:** This ensures:
- Referential integrity is never broken by LOD changes
- Projections are self-contained (no "fetch more detail" round-trips)
- Diff works correctly (all fields present in both snapshots)

**v2 Rule (future, optional):** Generator MAY strip fields by LOD for size optimization, but:
- MUST preserve all `$ref` targets (no dangling references)
- MUST preserve `id`, `kind`, `label_short` at all LOD levels
- MUST emit `lod_emitted: <level>` in envelope so renderer knows what's available

---

## 4. Node Model Specification

### 4.1 Base Node Structure

Every node MUST have:

```yaml
"<NodeId>":
  kind: <NodeKind>                    # Required. Discriminator for rendering
  id: <NodeId>                        # Required. Must match map key
  label_short: <string>               # Required. Human-readable label (≤60 chars)
```

Every node MAY have:

```yaml
  label_full: <string>                # Extended label with full details
  glyph: <string>                     # Emoji or icon identifier
  summary: { ... }                    # Kind-specific summary object
  tags: [<string>, ...]               # Freeform tags for filtering
  attributes: { ... }                 # Kind-specific attributes
  provenance: { ... }                 # Source/confidence metadata
  branches: { ... }                   # Named child $refs (for tree kinds)
  children: [{ $ref }, ...]           # Ordered child $refs (for list kinds)
  links: { ... }                      # Cross-references to related nodes
  list: { ... }                       # Paged list structure
  edges: { ... }                      # For graph kinds (control/holding edges)
  table: { ... }                      # For table kinds (matrix slices)
```

### 4.2 Node Kind Enumeration

| Kind | Wire Name | Description | Required Fields | Provenance Required |
|------|-----------|-------------|-----------------|---------------------|
| `CBU` | `"CBU"` | Client Business Unit root | `branches` | No |
| `MemberList` | `"MemberList"` | Paged list of CBU members | `list` | No |
| `Entity` | `"Entity"` | Legal entity | `entity_id`, `entity_kind` | No |
| `ProductTree` | `"ProductTree"` | Product taxonomy root | `children` | No |
| `Product` | `"Product"` | Product node | `children` | No |
| `Service` | `"Service"` | Service node | `children` | No |
| `Resource` | `"Resource"` | Resource leaf | — | No |
| `ProductBinding` | `"ProductBinding"` | Entity ↔ Product mapping | `bindings` | No |
| `InstrumentMatrix` | `"InstrumentMatrix"` | Matrix root | `axes`, `sparse_cells` | No |
| `MatrixSlice` | `"MatrixSlice"` | 2D slice of matrix | `table` | No |
| `SparseCellPage` | `"SparseCellPage"` | Paged sparse cell list | `sparse_cells` | No |
| `InvestorRegister` | `"InvestorRegister"` | Economic register root | `children` | No |
| `HoldingEdgeList` | `"HoldingEdgeList"` | Paged list of holding edges | `list` | No |
| `HoldingEdge` | `"HoldingEdge"` | Investor → Fund edge | `from`, `to`, `metrics` | **Yes** |
| `ControlRegister` | `"ControlRegister"` | Control/UBO register root | `children` | No |
| `ControlTree` | `"ControlTree"` | Control hierarchy root | `roots` | No |
| `ControlNode` | `"ControlNode"` | Entity in control tree | `entity`, `edges` | No |
| `ControlEdge` | `"ControlEdge"` | Controller → Controlled edge | `from`, `to`, `control_type` | **Yes** |

**Note:** `HoldingEdge` and `ControlEdge` MUST have `provenance`. This is enforced by the validator (see §9.1).

### 4.3 LOD Field Visibility Matrix

This table defines which fields the **renderer displays** at each LOD level. The generator emits all fields regardless of LOD (see §3.5).

| Field | LOD 0 | LOD 1 | LOD 2 | LOD 3 |
|-------|-------|-------|-------|-------|
| `id` | ✓ | ✓ | ✓ | ✓ |
| `kind` | — | ✓ | ✓ | ✓ |
| `glyph` | ✓ | ✓ | ✓ | ✓ |
| `label_short` | — | ✓ | ✓ | ✓ |
| `label_full` | — | — | — | ✓ |
| `tags` | — | — | ✓ | ✓ |
| `summary` | — | — | ✓ | ✓ |
| `branches` (collapsed) | — | — | ✓ | — |
| `branches` (expanded) | — | — | — | ✓ |
| `children` (count only) | — | — | ✓ | — |
| `children` (full) | — | — | — | ✓ |
| `attributes` | — | — | — | ✓ |
| `provenance` | — | — | — | ✓ |
| `links` | — | — | ✓ | ✓ |
| `metrics` | — | — | ✓ | ✓ |
| `confidence` | — | — | ✓ | ✓ |

### 4.4 Provenance Structure

```yaml
provenance:
  sources: [<string>, ...]            # Required. Origin systems/documents
  asserted_at: <ISO8601 date>         # Required. When assertion was made
  confidence: <float 0.0-1.0>         # Optional. Confidence score (default: 1.0 if omitted)
  notes: <string>                     # Optional. Human-readable notes
  evidence_refs: [{ $ref }, ...]      # Optional. Links to evidence nodes
```

**Compliance Requirement:** Every `HoldingEdge` and `ControlEdge` MUST have a `provenance` object with at least `sources` and `asserted_at`. The validator enforces this (see §9.1).

**Confidence Semantics:**
- `1.0` — Confirmed from authoritative source
- `0.8-0.99` — High confidence, minor ambiguity
- `0.5-0.79` — Medium confidence, needs verification
- `<0.5` — Low confidence, flagged for review

If `confidence` is omitted, assume `1.0` (no ambiguity).

### 4.5 Matrix Cell Value Schema

For consistent table rendering, sparse cell values follow this schema:

```yaml
# MatrixCellValue
value:
  enabled: <boolean>                  # Required. Whether capability is active
  source: <"refdata"|"dsl"|"override"|"default">  # Required. Origin of value
  default: <boolean>                  # Optional. True if this is the default value (not overridden)
  dsl_instruction_id: <string>        # Optional. ID of DSL instruction that set this
  dsl_run_id: <string>                # Optional. ID of DSL execution run
  policy_gate: <string>               # Optional. Policy rule that governs this cell
  notes_short: <string>               # Optional. Brief explanation
  provenance:                         # Optional. Source lineage
    sources: [<string>, ...]
    asserted_at: <ISO8601 date>
```

**Source Values:**
| Source | Meaning |
|--------|---------|
| `refdata` | Loaded from reference data system |
| `dsl` | Set by DSL instruction execution |
| `override` | Manually overridden by user/admin |
| `default` | System default (no explicit setting) |

---

## 5. Reference System Specification

### 5.1 NodeId Format

```
NodeId := <kind_prefix> ":" <qualifier>* ":" <identifier>

kind_prefix  := [a-z]+
qualifier    := <segment> (":" <segment>)*
identifier   := <alphanumeric_with_case>
segment      := [A-Za-z0-9_]+
```

**Key Point:** NodeIds allow mixed case after the kind prefix. This accommodates domain values like MIC codes (XLON, XEUR) and UUIDs.

**Examples:**
```
cbu:0
cbu:members:0
cbu:members:0:page:2
entity:uuid:fund_001
entity:uuid:im_001
products:root:0
products:product:custody
products:service:settlement
products:resource:swift
products:fund:fund_001
matrix:0
matrix:focus:mic:XLON          # Note: uppercase MIC preserved
matrix:focus:mic:XEUR
matrix:focus:entity:fund_001
matrix:sparse:page:2
register:economic:0
register:economic:holdings:for:fund_001
register:economic:edge:001
register:control:0
register:control:tree:root
register:control:node:fund_001
register:control:edge:001
```

**Validation Regex (Rust):**
```rust
lazy_static! {
    static ref NODE_ID_PATTERN: Regex = 
        Regex::new(r"^[a-z]+:[A-Za-z0-9_:]+$").unwrap();
}
```

**Note:** The kind prefix (before first `:`) is lowercase only. Subsequent segments allow uppercase to preserve domain values.

### 5.2 $ref Value Structure

```yaml
$ref: "<NodeId>"
```

**Resolution:** Given `{ $ref: "entity:uuid:fund_001" }`, resolve by looking up `nodes["entity:uuid:fund_001"]`.

**Error Handling:**
- Missing target → Validation error (pre-render)
- Malformed $ref → Validation error (pre-render)
- Cycle detected → Warning (render with cycle indicator)

### 5.3 $ref Contexts

| Context | Structure | Example |
|---------|-----------|---------|
| Single ref | `{ $ref: "<NodeId>" }` | `branches.members: { $ref: "cbu:members:0" }` |
| Ref in list | `[{ $ref: "<NodeId>" }, ...]` | `items: [{ $ref: "entity:uuid:e1" }]` |
| Named ref map | `{ <n>: { $ref: "<NodeId>" } }` | `investor_registers: { economic: { $ref: "..." } }` |
| Paging next | `{ paging: { next: "<NodeId or null>" } }` | `paging: { limit: 50, next: "cbu:members:0:page:2" }` |

### 5.4 Paging Evolution Path

**v1 (current):** `paging.next` is a `NodeId` string pointing to the next page node, or `null` if no more pages.

```yaml
paging:
  limit: 50
  next: "cbu:members:0:page:2"    # NodeId of next page node
```

**v2 (future, optional):** `paging.next` may be an opaque cursor token for server-side pagination:

```yaml
paging:
  limit: 50
  next: "eyJvZmZzZXQiOjUwfQ=="    # Opaque base64 cursor
  next_type: "cursor"             # Discriminator: "node_id" or "cursor"
```

**Implementation Note:** For v1, always treat `next` as a NodeId. The `next_type` field is reserved for future use.

---

## 6. Render Policy Specification

### 6.1 Full Structure

```yaml
render_policy:
  lod: <0|1|2|3>                      # Level of detail (default: 2)
  max_depth: <integer>                # Auto-expand depth from roots (default: 3)
  max_items_per_list: <integer>       # Virtualization threshold (default: 50)
  
  show:
    chambers: [<string>, ...]         # Which chambers to include
    branches: [<string>, ...]         # Which branch names to include
    node_kinds: [<NodeKind>, ...]     # Optional: whitelist of node kinds
  
  prune:
    exclude_paths: [<path_pattern>, ...]  # Glob patterns to exclude
    filters:
      <filter_name>: <filter_value>   # Kind-specific filters
```

### 6.2 Filter Specifications

| Filter | Applies To | Semantics |
|--------|------------|-----------|
| `member_role_any` | `MemberList` items | Include only entities with role in list |
| `member_role_none` | `MemberList` items | Exclude entities with role in list |
| `entity_kind_any` | `Entity` nodes | Include only specified entity kinds |
| `min_confidence` | Edges with `confidence` | Exclude edges below threshold |
| `has_provenance` | Any node | Include only nodes with provenance |

### 6.3 Path Pattern Syntax

```
path_pattern := <segment> ("." <segment>)* ("[*]")?

segment := <field_name> | "*"
```

**Examples:**
```
cbu.members[*].documents      # Exclude all documents from all members
register.control.edges[*]     # Exclude all control edges
*.provenance                  # Exclude provenance from all nodes
```

### 6.4 Policy Application Order

1. **Chamber filter** — Exclude entire chambers not in `show.chambers`
2. **Branch filter** — Exclude branches not in `show.branches`
3. **Node kind filter** — Exclude nodes not in `show.node_kinds` (if specified)
4. **Path exclusion** — Prune nodes matching `exclude_paths`
5. **Kind-specific filters** — Apply `filters` to matching node kinds
6. **Depth limit** — Mark nodes beyond `max_depth` as collapsed
7. **List virtualization** — Page lists exceeding `max_items_per_list`
8. **LOD application** — (v1: renderer only; v2: optionally strip fields)

---

## 7. Navigation Semantics

### 7.1 Focus Stack

The UI maintains a **focus stack** of visited nodes:

```rust
struct FocusStack {
    stack: Vec<NodeId>,
    current_index: usize,  // Pointer into stack for back/forward
}
```

**Operations:**
| Operation | Effect |
|-----------|--------|
| `push(node_id)` | Truncate stack after `current_index`, push new node, advance index |
| `back()` | Decrement `current_index` if > 0 |
| `forward()` | Increment `current_index` if < stack.len() - 1 |
| `jump_to(index)` | Set `current_index` to `index` (for breadcrumb clicks) |
| `current()` | Return `stack[current_index]` |

### 7.2 Breadcrumbs

Display path from root to current focus:

```
CBU: Allianz AM > Members (3) > Fund: Allianz IE ETF SICAV > Holdings
```

Each segment is clickable → `jump_to(index)`.

### 7.3 Expand/Collapse State

Per-session UI state (not in projection):

```rust
struct ExpansionState {
    expanded: HashSet<NodeId>,
    collapsed_override: HashSet<NodeId>,  // User explicitly collapsed
}
```

**Default:** Nodes auto-expand up to `max_depth` from root. User clicks toggle membership in `expanded`/`collapsed_override`.

### 7.4 Zoom Semantics (LOD + Depth)

| Action | Effect |
|--------|--------|
| **Zoom In** on node | Increase effective `lod` for subtree; increase `max_depth` for subtree |
| **Zoom Out** | Decrease effective `lod`; collapse to summaries |
| **Focus** on node | Push to focus stack; center UI on node |

**Implementation Note:** For Phase 0-2, zoom is UI-only (re-render with different display rules). For Phase 3+, zoom may trigger re-projection with different policy.

### 7.5 Pan Semantics

| Context | Pan Action | Effect |
|---------|------------|--------|
| List | Up/Down | Select previous/next item |
| List | Page Up/Down | Jump by `max_items_per_list` |
| Table | Arrow keys | Move cell selection |
| Table | Tab | Next column, wrap to next row |

### 7.6 Pinned Nodes

Users can pin frequently-accessed nodes:

```rust
struct PinnedNodes {
    pins: Vec<NodeId>,
    labels: HashMap<NodeId, String>,  // Optional custom labels
}
```

Pins appear in a sidebar or toolbar for quick access.

### 7.7 Deep Links (Phase 4)

URL scheme for external linking:

```
obpoc://snapshot/<snapshot_id>/node/<node_id>
obpoc://snapshot/<snapshot_id>/node/<node_id>?lod=3&expand=true
```

Use cases:
- Click from chat debug output to inspector
- Share specific node with colleague
- Bookmark for audit trail

---

## 8. Domain-Specific Patterns

### 8.1 CBU Container Pattern

```yaml
"cbu:0":
  kind: "CBU"
  id: "cbu:0"
  label_short: "CBU: Allianz AM — IE Platform"
  summary:
    members_count: 3
    products_count: 2
    matrix_rows: 4
    investor_registers: ["economic", "control"]
  branches:
    members: { $ref: "cbu:members:0" }
    products: { $ref: "products:root:0" }
    instrument_matrix: { $ref: "matrix:0" }
    investor_registers:
      economic: { $ref: "register:economic:0" }
      control:  { $ref: "register:control:0" }
```

**Rendering:** Tree node with expandable branches. Summary shown at LOD 2+.

### 8.2 Entity Pattern

```yaml
"entity:uuid:fund_001":
  kind: "Entity"
  id: "entity:uuid:fund_001"
  entity_id: "fund_001"
  entity_kind: "Fund"
  label_short: "Fund: Allianz IE ETF SICAV"
  label_full: "Allianz Ireland ETF SICAV plc (LEI: 549300IEDEMO000001)"
  glyph: "📦"
  tags: ["IE", "ETF", "UCITS"]
  links:
    products: [{ $ref: "products:fund:fund_001" }]
    matrix: { $ref: "matrix:focus:entity:fund_001" }
    holdings: { $ref: "register:economic:holdings:for:fund_001" }
    ubo: { $ref: "register:control:node:fund_001" }
```

**Rendering:** Detail card with glyph, labels, tags. Links section shows related nodes across chambers.

### 8.3 Instrument Matrix Pattern

```yaml
"matrix:0":
  kind: "InstrumentMatrix"
  id: "matrix:0"
  label_short: "Instrument Matrix"
  axes:
    mic: ["XLON", "XEUR", "XNYS"]
    instrument_type: ["EQUITY", "SWAP", "BOND"]
    capability: ["TRADE", "SETTLE", "CLEAR", "MARGIN"]
  sparse_cells:
    paging: { limit: 100, next: "matrix:sparse:page:2" }
    items:
      - key: ["XLON", "EQUITY", "SETTLE"]
        value:
          enabled: true
          source: "refdata"
          default: false
          notes_short: "standard settlement"
      - key: ["XLON", "SWAP", "MARGIN"]
        value:
          enabled: true
          source: "dsl"
          default: false
          dsl_instruction_id: "instr_001"
          notes_short: "VM CSA supported"
  focus:
    by_mic: { $ref: "matrix:focus:mic:XLON" }
    by_entity: { $ref: "matrix:focus:entity:fund_001" }
```

**Rendering:**
1. Summary view: Axis labels + cell count + focus slice links
2. Slice view: 2D table with row/column headers
3. Sparse view: Paged list of non-default cells

### 8.4 Matrix Slice Pattern

```yaml
"matrix:focus:mic:XLON":
  kind: "MatrixSlice"
  id: "matrix:focus:mic:XLON"
  label_short: "XLON slice"
  table:
    columns: ["instrument_type", "TRADE", "SETTLE", "MARGIN"]
    rows:
      - ["EQUITY", true, true, false]
      - ["SWAP", true, true, true]
```

**Rendering:** Standard table with header row. Boolean cells render as ✓/✗ or colored indicators.

### 8.5 Holding Edge Pattern

```yaml
"register:economic:edge:001":
  kind: "HoldingEdge"
  id: "register:economic:edge:001"
  label_short: "Pooled Vehicle A → Allianz IE ETF SICAV (2.1%)"
  from: { $ref: "entity:uuid:inv_001" }
  to: { $ref: "entity:uuid:fund_001" }
  metrics:
    pct: 2.1
    units: 120000
    nav_date: "2026-01-15"
  provenance:                         # REQUIRED for HoldingEdge
    sources: ["transfer_agent_registry_file"]
    asserted_at: "2026-01-20"
    confidence: 0.90
```

**Rendering:** Edge card with from/to entity links, metrics table, provenance panel.

### 8.6 Control Edge Pattern (UBO)

```yaml
"register:control:edge:001":
  kind: "ControlEdge"
  id: "register:control:edge:001"
  label_short: "IM controls board (policy 51%)"
  from: { $ref: "entity:uuid:im_001" }
  to: { $ref: "entity:uuid:fund_001" }
  control_type: "BoardControl"
  confidence: 0.92
  ambiguity_flags: []
  provenance:                         # REQUIRED for ControlEdge
    sources: ["kyc_doc", "corporate_registry"]
    asserted_at: "2026-01-10"
```

**Control Types:**
| Type | Description |
|------|-------------|
| `BoardControl` | Controls board composition |
| `MajorityShareholder` | >50% voting rights |
| `SignificantInfluence` | 25-50% or other influence |
| `Nominee` | Holds on behalf of another |
| `TrustBeneficiary` | Beneficiary of trust structure |

**Ambiguity Flags:**
| Flag | Meaning |
|------|---------|
| `circular_reference` | Cycle detected in control chain |
| `conflicting_sources` | Sources disagree on control relationship |
| `stale_data` | Assertion older than threshold |
| `low_confidence` | Confidence below threshold |

### 8.7 Control Tree Pattern (UBO Hierarchy)

```yaml
"register:control:tree:root":
  kind: "ControlTree"
  id: "register:control:tree:root"
  label_short: "Control tree"
  roots:
    - { $ref: "register:control:node:fund_001" }

"register:control:node:fund_001":
  kind: "ControlNode"
  id: "register:control:node:fund_001"
  label_short: "Allianz IE ETF SICAV"
  entity: { $ref: "entity:uuid:fund_001" }
  depth: 0
  edges:
    controlled_by:
      - { $ref: "register:control:edge:001" }
    controls: []
```

**Rendering:** Tree with entities as nodes, control edges as branches. Ambiguity flags shown as warning indicators.

---

## 9. Validation Requirements

### 9.1 Pre-Render Validation

The validator MUST check:

| Check | Error Level | Message Pattern |
|-------|-------------|-----------------|
| Schema version supported | Error | `"Schema version {v} not supported (max: {max})"` |
| Required envelope fields present | Error | `"Missing required field: snapshot.{field}"` |
| All `$ref` targets exist in `nodes` | Error | `"Broken reference: {ref} in {context} (target not found)"` |
| NodeId format valid | Error | `"Invalid node ID format: {id}"` |
| Node `id` matches map key | Error | `"Node ID mismatch: key={key}, id={id}"` |
| Required fields for kind present | Error | `"Node {id} (kind={kind}) missing required field: {field}"` |
| `HoldingEdge` has `provenance` | Error | `"HoldingEdge {id} missing required provenance"` |
| `ControlEdge` has `provenance` | Error | `"ControlEdge {id} missing required provenance"` |
| `provenance.sources` non-empty | Error | `"Node {id} provenance.sources is empty"` |
| `provenance.asserted_at` present | Error | `"Node {id} provenance missing asserted_at"` |
| Paging `next` targets exist | Error | `"Broken paging reference: {next} in {context}"` |
| No orphan nodes (unreachable from root) | Warning | `"Orphan node: {id} (not reachable from any root)"` |
| `confidence` in valid range [0.0, 1.0] | Warning | `"Node {id} confidence {val} outside valid range"` |

### 9.2 Cycle Detection

Control and holding graphs may contain cycles (legitimate in complex ownership structures). The validator MUST:

1. Detect cycles in `ControlEdge` chains
2. Detect cycles in `HoldingEdge` chains
3. Report cycles as **warnings** (not errors)
4. Annotate cycle-participating nodes with `cycle_participant: true`

**Algorithm:** DFS from each root, track visited set per path, report back-edges.

### 9.3 Validation Output Format

```yaml
validation_result:
  valid: <boolean>
  errors:
    - level: "error"
      code: "BROKEN_REF"
      message: "Broken reference: entity:uuid:missing in cbu:members:0.list.items[1]"
      context:
        node_id: "cbu:members:0"
        field: "list.items[1]"
        ref_value: "entity:uuid:missing"
  warnings:
    - level: "warning"
      code: "CYCLE_DETECTED"
      message: "Cycle detected in control graph: fund_A → im_A → holding_A → fund_A"
      context:
        cycle_path: ["register:control:node:fund_A", "register:control:node:im_A"]
    - level: "warning"
      code: "ORPHAN_NODE"
      message: "Orphan node: entity:uuid:unused_001 (not reachable from any root)"
      context:
        node_id: "entity:uuid:unused_001"
```

### 9.4 Orphan Node Handling

**Validator behavior:** Report orphan nodes as warnings (see above).

**Renderer behavior:**
- **Default:** Hide orphan nodes from navigation tree
- **Diagnostics mode:** Show orphan nodes in a separate "Orphans" section at the bottom of the tree, styled with warning indicator
- **Toggle:** UI provides "Show orphans" toggle in settings

**Use case:** Orphan nodes indicate generator/policy bugs. Showing them in diagnostics mode aids debugging.

### 9.5 Validation Integration Points

| Point | Behavior |
|-------|----------|
| File load | Validate immediately; show errors in UI; block render if errors |
| Generator output | Validate before write; fail generation if errors |
| CLI tool | `obpoc inspect validate <file>` returns exit code 0/1 |

---

## 10. UI Shell Specification

### 10.1 Layout

```
┌─────────────────────────────────────────────────────────────────────┐
│ [Search: ____________________] [LOD: ▼] [Depth: ▼] [Filters: ▼]    │
├─────────────────────────────────────────────────────────────────────┤
│ Breadcrumb: CBU > Members > Fund: Allianz IE ETF SICAV              │
├───────────────────┬─────────────────────────┬───────────────────────┤
│                   │                         │                       │
│   Navigation      │     Main View           │    Detail Pane        │
│   Tree            │     (tree/table/card)   │    (selected node)    │
│                   │                         │                       │
│   - CBU           │                         │   Kind: Entity        │
│     - Members     │   [Table view of        │   ID: entity:uuid:... │
│       - Fund ←    │    current focus]       │   Label: Fund: ...    │
│       - IM        │                         │   Tags: IE, ETF       │
│       - Cust      │                         │                       │
│     - Products    │                         │   Links:              │
│     - Matrix      │                         │   - Products (2)      │
│     - Registers   │                         │   - Matrix slice      │
│                   │                         │   - Holdings          │
│   ──────────────  │                         │                       │
│   Pinned:         │                         │   Provenance:         │
│   - Fund_001      │                         │   - Sources: [...]    │
│                   │                         │   - Asserted: 2026-01 │
│   ──────────────  │                         │                       │
│   Orphans: (0)    │                         │                       │
│   [Show orphans]  │                         │                       │
│                   │                         │                       │
└───────────────────┴─────────────────────────┴───────────────────────┘
│ Status: Loaded inspector_projection_sample.yaml | Nodes: 27 | Valid │
└─────────────────────────────────────────────────────────────────────┘
```

### 10.2 Interaction Patterns

| Element | Click | Double-Click | Right-Click |
|---------|-------|--------------|-------------|
| Tree node | Select + show in detail | Expand/collapse | Context menu (pin, copy ID, focus) |
| `$ref` link | Navigate (push to stack) | — | Context menu (open in new tab) |
| Table cell | Select cell | Edit (if editable) | Context menu (copy value) |
| Breadcrumb segment | Jump to ancestor | — | — |
| Pinned node | Navigate | — | Unpin |

### 10.3 Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate tree / list |
| `←` / `→` | Collapse / Expand |
| `Enter` | Select / Follow $ref |
| `Backspace` / `Alt+←` | Back in history |
| `Alt+→` | Forward in history |
| `/` or `Ctrl+F` | Focus search |
| `Ctrl+P` | Pin current node |
| `1` / `2` / `3` / `4` | Set LOD 0/1/2/3 |
| `Escape` | Clear selection / Close modal |

### 10.4 Search Specification

**Search Modes:**

| Mode | Syntax | Matches |
|------|--------|---------|
| Default | `<text>` | `label_short`, `label_full`, `id` (substring) |
| ID exact | `id:<node_id>` | `id` (exact) |
| Entity ID | `entity:<entity_id>` | `entity_id` (exact) |
| Kind | `kind:<kind>` | `kind` (exact) |
| Tag | `tag:<tag>` | `tags` (contains) |

**Results:** List of matching nodes with glyph + label_short. Click to navigate.

---

## 11. Implementation Plan

### 11.1 Generator Source-of-Truth Roadmap

To ensure the inspector is a "faithful representation" (not a "pretty lie"), the generator evolves through phases:

| Phase | Source | Description |
|-------|--------|-------------|
| Phase 1 | Nested Rust structs | Walk in-memory snapshot graph directly; fast but coupled to struct layout |
| Phase 2+ | Flat snapshot arrays | Generate from serialized/persisted chamber arrays; decoupled, authoritative |
| Parity tests | Both | Projection-from-struct MUST match projection-from-snapshot for key views |

**Parity Test Strategy:**
1. Generate projection from nested structs → `proj_struct.yaml`
2. Serialize snapshot to flat arrays, reload, generate projection → `proj_flat.yaml`
3. Diff `proj_struct.yaml` vs `proj_flat.yaml`; must be identical (after deterministic ordering)

This ensures the inspector never diverges from the canonical data model.

---

### Phase 0: Foundation (Target: 1 week)

**Goal:** Load fixture, navigate `$ref`, see details.

**Deliverables:**
1. `model.rs` — Structs for `InspectorProjection`, `Node`, `RenderPolicy` with serde
2. `node_id.rs` — `NodeId` newtype with validation (mixed-case after prefix)
3. `ref_value.rs` — `RefValue` newtype with resolution
4. `validate.rs` — Referential integrity check with friendly errors
5. EGUI shell — Three-panel layout, load file, tree render, detail pane
6. Basic navigation — Click `$ref` to navigate, back button

**Acceptance Criteria:**
- [ ] `cargo run -- inspect load fixtures/inspector_projection_sample.yaml` opens UI
- [ ] Tree shows CBU with expandable branches
- [ ] Clicking entity `$ref` updates detail pane
- [ ] Back button returns to previous node
- [ ] Loading `invalid_ref.yaml` shows validation errors (including provenance checks)

**Test Fixtures:**
- `inspector_projection_sample.yaml` — Happy path
- `inspector_projection_invalid_ref.yaml` — Validation errors

---

### Phase 1: Projection Generator (Target: 1 week)

**Goal:** Generate projection from live snapshot.

**Deliverables:**
1. `generator.rs` — Walk snapshot graph, emit `InspectorProjection`
2. `labels.rs` — Shorthand label generation rules
3. CLI command — `obpoc inspect generate --snapshot <file> --output <file>`
4. Deterministic ordering — Use `BTreeMap`, sort arrays per §3.4

**Acceptance Criteria:**
- [ ] Generate projection from test snapshot
- [ ] Generated projection passes validation (including provenance on edges)
- [ ] Generated projection renders correctly in UI
- [ ] Labels follow shorthand rules
- [ ] Re-running generator on same input produces byte-identical output

**Test Cases:**
- Minimal CBU (1 member)
- Full CBU (all chambers)
- Deep UBO hierarchy (5+ levels)

---

### Phase 2: Render Policy (Target: 1 week)

**Goal:** Apply filters, LOD, depth limits.

**Deliverables:**
1. `policy.rs` — Policy application logic
2. UI controls — LOD dropdown, depth slider, filter toggles
3. Re-projection on policy change (or UI-only filtering)

**Acceptance Criteria:**
- [ ] Changing LOD updates field visibility (renderer respects LOD matrix)
- [ ] Changing max_depth updates auto-expand behavior
- [ ] Filters exclude matching nodes
- [ ] `exclude_paths` prunes specified branches
- [ ] Policy changes don't break `$ref` integrity
- [ ] Orphan nodes shown in diagnostics mode

**Test Cases:**
- LOD 0 shows only glyphs + IDs
- Filter `member_role_any: [Fund]` shows only funds
- `exclude_paths: [cbu.members[*].documents]` hides documents

---

### Phase 3: Matrix & Table Rendering (Target: 1 week)

**Goal:** Proper table rendering with virtualization.

**Deliverables:**
1. `render/table.rs` — Table renderer with header, row selection
2. Matrix slice navigation — Switch between `by_mic`, `by_entity` slices
3. Sparse cell paging — Load next page on scroll
4. `MatrixCellValue` rendering — Show source, provenance, DSL links

**Acceptance Criteria:**
- [ ] `MatrixSlice` renders as table with headers
- [ ] Cell selection with arrow keys
- [ ] Slice switching via dropdown or links
- [ ] Paging loads more cells without freezing UI
- [ ] Cell value tooltip shows source + provenance

**Test Fixtures:**
- `inspector_projection_matrix_heavy.yaml`

---

### Phase 4: Deep Links (Target: 3 days)

**Goal:** External linking into specific nodes.

**Deliverables:**
1. URL scheme registration — `obpoc://` handler
2. Link generation — Right-click → "Copy deep link"
3. Link handling — Parse URL, load projection, navigate to node

**Acceptance Criteria:**
- [ ] Copy link from UI
- [ ] Open link from terminal/browser navigates to node
- [ ] Invalid links show friendly error

---

### Phase 5: Diff Support (Target: 1 week)

**Goal:** Compare two projections, show changes.

**Deliverables:**
1. `diff.rs` — Structural diff algorithm
2. Diff UI — Side-by-side or inline change indicators
3. CLI command — `obpoc inspect diff <before> <after>`

**Acceptance Criteria:**
- [ ] Detect added/removed/modified nodes
- [ ] Show field-level changes for modified nodes
- [ ] Highlight changes in tree view
- [ ] Export diff as report (Markdown or JSON)

**Test Fixtures:**
- `inspector_projection_diff_before.yaml`
- `inspector_projection_diff_after.yaml`

---

## 12. Risk Analysis

### 12.1 Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| egui tree performance at scale | Medium | High | Virtualize tree; test with 1000+ node fixture |
| Serde YAML edge cases | Low | Medium | Comprehensive test fixtures; explicit schema |
| Cycle detection complexity | Low | Medium | Well-known DFS algorithm; test with adversarial fixtures |
| Policy application ordering bugs | Medium | Medium | Explicit ordering spec; property-based testing |
| NodeId case sensitivity bugs | Medium | Medium | Strict regex; test with mixed-case MICs |
| Determinism failures | Medium | High | Use BTreeMap; test byte-identical regeneration |

### 12.2 Scope Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Feature creep (add astronomy back too early) | Medium | High | Hard phase gate; astronomy is Phase 6+ |
| Over-engineering policy DSL | Medium | Medium | Start with fixed filter set; extensibility later |
| Diff complexity explosion | Low | Medium | Phase 5 is optional; can ship without |

### 12.3 Integration Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Snapshot schema changes break generator | Medium | High | Version both schemas; migration tooling |
| UI state management complexity | Medium | Medium | Keep UI state separate from projection; immutable projection |
| Generator diverges from snapshot truth | Medium | High | Parity tests (§11.1); CI enforcement |

---

## 13. Open Questions

### 13.1 Schema & Contract

1. **Should `NodeId` be typed?** Currently string with regex validation. Alternative: `NodeId { kind: NodeKind, qualifiers: Vec<String>, id: String }`. Trade-off: type safety vs. serialization simplicity.

2. **Should `$ref` support fragments?** E.g., `{ $ref: "entity:uuid:fund_001#provenance" }` to link to a specific field. Use case: provenance cross-references. Complexity: requires field-level addressing.

3. **How to handle schema evolution?** When snapshot schema changes, generator must change. Should projections be forward-compatible (ignore unknown fields) or strict?

### 13.2 Navigation & UI

4. **Should zoom be per-node or global?** Current spec: global LOD/depth. Alternative: per-subtree overrides. Trade-off: simplicity vs. flexibility.

5. **Should pinned nodes persist across sessions?** Current: session-only. Alternative: persist in user preferences. Requires: storage mechanism.

6. **How to handle very deep trees?** UBO chains can be 10+ levels. Options: virtual scroll, lazy load, breadcrumb-only past threshold.

### 13.3 Performance

7. **At what node count does the UI degrade?** Need benchmark. Target: 1000 nodes smooth, 10000 nodes acceptable with virtualization.

8. **Should projections be pre-generated or on-demand?** Current: pre-generated file. Alternative: generate on load from snapshot. Trade-off: startup time vs. file management.

### 13.4 Future

9. **How does this integrate with the DSL REPL?** Use case: Execute DSL command, see projection update. Requires: live re-projection or incremental update.

10. **How does this integrate with GLEIF discovery?** Use case: GLEIF returns new entity, appears in projection. Requires: projection merge or regeneration.

---

## 14. Appendices

### Appendix A: Complete Fixture Inventory

| Fixture | Purpose | Key Patterns Tested |
|---------|---------|---------------------|
| `inspector_projection_sample.yaml` | End-to-end happy path | All chambers, all node kinds, basic paging |
| `inspector_projection_stress.yaml` | Paging / virtualization | Large member list with pagination |
| `inspector_projection_matrix_heavy.yaml` | Table rendering | Matrix axes, sparse cells, slices |
| `inspector_projection_ubo_heavy.yaml` | Deep control tree | Multi-level UBO, edge confidence |
| `inspector_projection_taxonomy_heavy.yaml` | Large product tree | Deep Product→Service→Resource nesting |
| `inspector_projection_diff_before.yaml` | Diff baseline | Snapshot before changes |
| `inspector_projection_diff_after.yaml` | Diff comparison | Snapshot after changes |
| `inspector_projection_invalid_ref.yaml` | Validation errors | Broken refs, malformed IDs, missing provenance |

### Appendix B: Glyph Vocabulary

| Kind | Glyph | Rationale |
|------|-------|-----------|
| CBU | 🏦 | Institution/bank |
| MemberList | 👥 | Group of people/entities |
| Entity (Fund) | 📦 | Container of assets |
| Entity (IM) | 🧭 | Navigator/manager |
| Entity (Custodian) | 🏛️ | Institution |
| Entity (Investor) | 💠 | Diamond/value |
| Product | 🧱 | Building block |
| Service | 🔁 | Process/cycle |
| Resource | 📨 | Connection/message |
| InstrumentMatrix | 🧮 | Calculation/grid |
| MatrixSlice | 📋 | Clipboard/table |
| InvestorRegister | 💰 | Money/economics |
| HoldingEdge | ➡️ | Direction/flow |
| ControlRegister | 🧬 | DNA/hierarchy |
| ControlTree | 🌳 | Tree structure |
| ControlNode | (inherits from entity) | — |
| ControlEdge | 🪢 | Knot/binding |
| ProductBinding | 🔗 | Link |

### Appendix C: Error Code Reference

| Code | Level | Description |
|------|-------|-------------|
| `UNSUPPORTED_SCHEMA_VERSION` | Error | Schema version newer than viewer supports |
| `MISSING_REQUIRED_FIELD` | Error | Required envelope or node field missing |
| `BROKEN_REF` | Error | `$ref` target not found in `nodes` |
| `INVALID_NODE_ID_FORMAT` | Error | NodeId doesn't match expected pattern |
| `NODE_ID_MISMATCH` | Error | Node's `id` field doesn't match map key |
| `MISSING_KIND_FIELD` | Error | Node missing required field for its kind |
| `BROKEN_PAGING_REF` | Error | Paging `next` target not found |
| `MISSING_PROVENANCE` | Error | `HoldingEdge` or `ControlEdge` missing provenance |
| `EMPTY_PROVENANCE_SOURCES` | Error | Provenance `sources` array is empty |
| `MISSING_ASSERTED_AT` | Error | Provenance missing `asserted_at` field |
| `ORPHAN_NODE` | Warning | Node not reachable from any root |
| `CYCLE_DETECTED` | Warning | Cycle found in control/holding graph |
| `INVALID_CONFIDENCE` | Warning | Confidence value outside [0.0, 1.0] range |
| `LOW_CONFIDENCE_EDGE` | Info | Edge confidence below threshold |
| `STALE_PROVENANCE` | Info | Provenance date older than threshold |

### Appendix D: Rust Type Sketches

```rust
// node_id.rs
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};

lazy_static! {
    /// NodeId pattern: lowercase prefix, then mixed-case segments
    /// Examples: "cbu:0", "matrix:focus:mic:XLON", "entity:uuid:fund_001"
    static ref NODE_ID_PATTERN: Regex = 
        Regex::new(r"^[a-z]+:[A-Za-z0-9_:]+$").unwrap();
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(String);

impl NodeId {
    pub fn parse(s: &str) -> Result<Self, NodeIdError> {
        if NODE_ID_PATTERN.is_match(s) {
            Ok(Self(s.to_string()))
        } else {
            Err(NodeIdError::InvalidFormat(s.to_string()))
        }
    }
    
    pub fn kind_prefix(&self) -> &str {
        self.0.split(':').next().unwrap_or("")
    }
    
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ref_value.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefValue {
    #[serde(rename = "$ref")]
    pub target: NodeId,
}

// model.rs
use std::collections::BTreeMap;  // BTreeMap for deterministic ordering

#[derive(Debug, Serialize, Deserialize)]
pub struct InspectorProjection {
    pub snapshot: SnapshotMeta,
    pub render_policy: RenderPolicy,
    pub ui_hints: UiHints,
    pub root: BTreeMap<String, RefValue>,
    pub nodes: BTreeMap<NodeId, Node>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Node {
    pub kind: NodeKind,
    pub id: NodeId,
    pub label_short: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_full: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glyph: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    // ... other optional fields
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Provenance {
    pub sources: Vec<String>,
    pub asserted_at: String,  // ISO8601 date
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<RefValue>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeKind {
    #[serde(rename = "CBU")]
    Cbu,
    #[serde(rename = "MemberList")]
    MemberList,
    #[serde(rename = "Entity")]
    Entity,
    #[serde(rename = "ProductTree")]
    ProductTree,
    #[serde(rename = "Product")]
    Product,
    #[serde(rename = "Service")]
    Service,
    #[serde(rename = "Resource")]
    Resource,
    #[serde(rename = "ProductBinding")]
    ProductBinding,
    #[serde(rename = "InstrumentMatrix")]
    InstrumentMatrix,
    #[serde(rename = "MatrixSlice")]
    MatrixSlice,
    #[serde(rename = "SparseCellPage")]
    SparseCellPage,
    #[serde(rename = "InvestorRegister")]
    InvestorRegister,
    #[serde(rename = "HoldingEdgeList")]
    HoldingEdgeList,
    #[serde(rename = "HoldingEdge")]
    HoldingEdge,
    #[serde(rename = "ControlRegister")]
    ControlRegister,
    #[serde(rename = "ControlTree")]
    ControlTree,
    #[serde(rename = "ControlNode")]
    ControlNode,
    #[serde(rename = "ControlEdge")]
    ControlEdge,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MatrixCellValue {
    pub enabled: bool,
    pub source: CellSource,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsl_instruction_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsl_run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_gate: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes_short: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CellSource {
    Refdata,
    Dsl,
    Override,
    Default,
}
```

### Appendix E: Validation Pseudocode

```rust
fn validate(projection: &InspectorProjection) -> ValidationResult {
    let mut errors = vec![];
    let mut warnings = vec![];
    
    // 1. Check schema version
    if projection.snapshot.schema_version > MAX_SUPPORTED_VERSION {
        errors.push(error("UNSUPPORTED_SCHEMA_VERSION", ...));
    }
    
    // 2. Collect all $ref targets
    let all_refs = collect_all_refs(projection);
    
    // 3. Check each ref exists
    for (context, ref_value) in all_refs {
        if !projection.nodes.contains_key(&ref_value.target) {
            errors.push(error("BROKEN_REF", context, ref_value));
        }
    }
    
    // 4. Check node IDs match keys
    for (key, node) in &projection.nodes {
        if key != &node.id {
            errors.push(error("NODE_ID_MISMATCH", key, node.id));
        }
    }
    
    // 5. Check required fields per kind
    for (id, node) in &projection.nodes {
        check_required_fields(id, node, &mut errors);
    }
    
    // 6. Check provenance requirements
    for (id, node) in &projection.nodes {
        if matches!(node.kind, NodeKind::HoldingEdge | NodeKind::ControlEdge) {
            if node.provenance.is_none() {
                errors.push(error("MISSING_PROVENANCE", id));
            } else {
                let prov = node.provenance.as_ref().unwrap();
                if prov.sources.is_empty() {
                    errors.push(error("EMPTY_PROVENANCE_SOURCES", id));
                }
                if prov.asserted_at.is_empty() {
                    errors.push(error("MISSING_ASSERTED_AT", id));
                }
            }
        }
    }
    
    // 7. Check confidence ranges
    for (id, node) in &projection.nodes {
        if let Some(ref prov) = node.provenance {
            if let Some(conf) = prov.confidence {
                if conf < 0.0 || conf > 1.0 {
                    warnings.push(warning("INVALID_CONFIDENCE", id, conf));
                }
            }
        }
    }
    
    // 8. Detect orphan nodes
    let reachable = compute_reachable_from_roots(projection);
    for id in projection.nodes.keys() {
        if !reachable.contains(id) {
            warnings.push(warning("ORPHAN_NODE", id));
        }
    }
    
    // 9. Detect cycles in control/holding graphs
    let cycles = detect_cycles(projection);
    for cycle in cycles {
        warnings.push(warning("CYCLE_DETECTED", cycle));
    }
    
    ValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}
```

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-02-04 | Adam | Initial consolidated doc |
| 2.0 | 2026-02-04 | Adam + Claude | Added: LOD field matrix, validation spec, UI shell spec, risk analysis, open questions, type sketches, expanded domain patterns |
| 3.0 | 2026-02-04 | Adam + Claude + ChatGPT | Fixed: NodeId regex (mixed case), NodeKind serde (explicit rename), clarified inline vs $ref rule, LOD emission strategy, provenance validation requirements, paging evolution path. Added: Deterministic ordering rules, MatrixCellValue schema, generator parity testing roadmap, orphan node handling, validation pseudocode |

---

## Peer Review Checklist (for implementer)

Before starting implementation, confirm:

- [ ] NodeId regex allows uppercase after prefix (e.g., `matrix:focus:mic:XLON`)
- [ ] NodeKind serde uses explicit `#[serde(rename = "CBU")]` style
- [ ] Understand inline value objects vs $ref distinction (§2.3)
- [ ] Generator emits all fields; renderer applies LOD (§3.5)
- [ ] `HoldingEdge` and `ControlEdge` provenance is required and validated
- [ ] Use `BTreeMap` for deterministic ordering
- [ ] Parity tests planned for generator (§11.1)
- [ ] Orphan handling: hide by default, show in diagnostics mode

---

*End of specification.*
