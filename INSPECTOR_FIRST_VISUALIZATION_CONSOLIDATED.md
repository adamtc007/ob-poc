# Inspector-First Visualization — Consolidated Architecture + TODO Pack + Projection Examples

**Context:** OB-POC has several dense composite structs (CBU container, Instrument Matrix, Investor Registers / UBO control hierarchy, Product ↔ Service ↔ Resource taxonomy).  
The existing EGUI “Astronomy/Esper” visualization is powerful but high-risk to tune. This document consolidates the revised **Inspector-first** approach: a deterministic YAML/JSON projection UI with expand/collapse, zoom/LOD, pan, and backtracking — plus fixtures and an implementation plan.

---

## 1. Problem Statement → “So we need XYZ”

### A) Astronomy UI is high-risk and slow to tune
- LOD/animation/picking/perf tuning is hard and time-consuming.
- UI iteration costs are high; correctness and explainability matter more than cinematic polish.

**So we need:** a lower-complexity inspector UI that is faithful, explainable, and fast to iterate.

### B) Structs are dense and can’t be dumped raw
- CBU/UBO/Matrix/Taxonomy can be huge; naïvely expanding everything explodes payloads and UI.

**So we need:** a projection contract with stable IDs, `$ref` links, paging, pruning and filters, and shorthand labels.

### C) Users still need navigation semantics (zoom/pan/back)
- Even without 3D, users must be able to “zoom into” detail and “back out”.

**So we need:** a deterministic navigation stack: focus stack + breadcrumbs + lens presets + LOD/depth controls.

### D) Must preserve bridge back to Astronomy later
**So we need:** projections as first-class outputs from the snapshot model (inspector is one projection; astronomy is another).

---

## 2. Architecture Target

### 2.1 Projection Graph + Render Policy
- **Projection Graph:** `nodes: Map<NodeId, Node>` and `root` refs; links are `$ref`.
- **Render Policy:** controls visibility + verbosity:
  - `lod` (verbosity)
  - `max_depth` (auto-expand depth)
  - `max_items_per_list` (virtualization)
  - `show` / `prune` filters and excluded paths

### 2.2 Projection envelope (contract)
```yaml
snapshot:
  schema_version: 9
  source_hash: "..."
  policy_hash: "..."
  created_at: "..."
render_policy:
  lod: 2
  max_depth: 3
  max_items_per_list: 50
  show:
    chambers: ["cbu","products","matrix","registers"]
    branches: ["members","products","instrument_matrix","investors"]
  prune:
    exclude_paths: ["cbu.members[*].documents"]
    filters:
      member_role_any: ["Fund","InvestmentManager"]
ui_hints:
  shorthand_labels: true
  breadcrumb: true
  history: true
root:
  cbu: { $ref: "cbu:0" }
nodes: { ... }
```

### 2.3 `$ref` contract
```yaml
$ref: "register:control:node:fund_001"
```
- Stable, explicit IDs. No implicit nesting required.

### 2.4 Paging / virtualization contract
```yaml
list:
  paging:
    limit: 50
    next: "opaque-token-or-node-id"
  items:
    - { $ref: "entity:uuid:..." }
```
> For fixtures, `next` can be the NodeId of the next page. In production it can be opaque.

---

## 3. The Structs (what we must visualize)

### 3.1 CBU Container (the trunk)
- Root container: members, roles/tags, product bindings, links out to matrix and registers.

### 3.2 Product ↔ Service ↔ Resource taxonomy
- Tree navigation, cross-links back into CBU usage and entity bindings.

### 3.3 Instrument Matrix
- Dense grid best rendered as:
  - axes definitions
  - sparse cell list
  - precomputed focus slices (`by_mic`, `by_entity`, optionally `by_type`)

### 3.4 Investor Registers
Two “lenses”:
- **Economic:** holdings edges (investor → fund) with provenance
- **Control/UBO:** control edges (controller → controlled) with provenance + ambiguity flags

---

## 4. Navigation semantics (Esper-lite without astronomy)

### Expand / Collapse
- Tree nodes expand/collapse; large lists use paging.

### Zoom
- **Zoom in:** increase `lod` and/or `max_depth` for focused node.
- **Zoom out:** decrease `lod` and/or collapse to summaries.

### Pan
- List: next/prev item selection.
- Table: next/prev row/col selection, or switch slice.

### Back out
- Focus stack: follow `$ref` pushes; Back pops.
- Breadcrumbs allow jump to any ancestor.
- “Pinned” nodes for quick access.

---

## 5. Node Model (required fields)

Each `Node` should support:
- `id`, `kind`, `label_short` (required), `label_full` (optional), `glyph` (optional)
- `branches/links` containing `$ref` targets
- Optional `summary`, `attributes`, `provenance`

Common kinds:
- `CBU`, `MemberList`, `Entity`
- `ProductTree`, `Product`, `Service`, `Resource`, `ProductBinding`
- `InstrumentMatrix`, `MatrixSlice`, `SparseCellPage`
- `InvestorRegister`, `HoldingEdgeList`, `HoldingEdge`
- `ControlRegister`, `ControlTree`, `ControlNode`, `ControlEdge`

---

## 6. Example Projections (conceptual patterns)

### 6.1 CBU trunk with branches as refs
```yaml
"cbu:0":
  kind: "CBU"
  label_short: "CBU: Allianz AM — IE Funds"
  branches:
    members: { $ref: "cbu:members:0" }
    products: { $ref: "products:root:0" }
    instrument_matrix: { $ref: "matrix:0" }
    investor_registers:
      economic: { $ref: "register:economic:0" }
      control:  { $ref: "register:control:0" }
```

### 6.2 Matrix slices (table-friendly)
```yaml
"matrix:focus:mic:XLON":
  kind: "MatrixSlice"
  table:
    columns: ["instrument_type","TRADE","SETTLE","CLEAR","MARGIN"]
    rows:
      - ["EQUITY", true, true, true, false]
      - ["SWAP",   true, true, true, true]
```

### 6.3 Investor register edges with provenance
```yaml
"register:economic:edge:001":
  kind: "HoldingEdge"
  from: { $ref: "entity:uuid:inv_001" }
  to:   { $ref: "entity:uuid:fund_001" }
  metrics: { pct: 2.1, units: 120000 }
  provenance: { sources: ["ta_registry"], asserted_at: "2026-01-20", confidence: 0.90 }
```

---

## 7. Implementation Plan (Phased)

### Phase 0 — Projection schema + minimal UI
1) Implement `InspectorProjection` Rust structs + serde YAML/JSON.
2) EGUI shell: selector (left), view (center), details (right), search (top).
3) Load projection from file and navigate `$ref`.

**Acceptance:** open fixture, click links, see details.

### Phase 1 — Projection generator (fast path, from nested structs)
- Emit trunk nodes + lists + slices.
- Add shorthand label rules (`label_short`).

**Acceptance:** generate and render a real CBU snapshot projection.

### Phase 2 — Render Policy (show/prune/filters + LOD/depth)
- Apply filters and excluded paths deterministically.
- LOD rules:
  - 0: icon + id
  - 1: short label
  - 2: normal
  - 3: verbose details

**Acceptance:** toggling policy changes visibility without breaking refs.

### Phase 3 — Matrix/table virtualization + focus navigation
- Table slices render well; pan/zoom works; sparse cell paging works.

### Phase 4 — Deep links (optional)
- `obpoc://snapshot/<id>/node/<node_id>` open focused node.
- Click from chat debug to inspector.

### Phase 5 — Diff (optional)
- Load before/after projections and show node/field-level changes.

---

## 8. TODO Pack Summary (drop to Claude Code)

**Primary file:** `docs/todo/INSPECTOR_FIRST_VISUALIZATION_TODO.md`

Key deliverables:
- Projection schema + validator (ensure all `$ref` targets exist; readable errors)
- EGUI inspector UI (tree + table + details + search)
- Navigation (focus stack + breadcrumbs + pan/zoom)
- Optional diff support

Suggested module layout:
```
rust/src/ui/inspector/
  model.rs        # InspectorProjection + serde
  validate.rs     # ref validator + friendly errors
  generator.rs    # projection generator (Phase 1)
  policy.rs       # RenderPolicy application
  render.rs       # EGUI renderers
  navigation.rs   # focus stack + pan/zoom + breadcrumbs
  labels.rs       # shorthand labels
  diff.rs         # optional
```

---

## 9. Fixture Pack (complete set)

A complete set of projection fixtures exists to test all use cases:
- sample (end-to-end)
- stress (paging/virtualization)
- matrix-heavy (table + sparse paging)
- ubo-heavy (deep control tree + ambiguity)
- taxonomy-heavy (large product/service/resource tree)
- diff pair (before/after)
- invalid refs (validator error UX)

**Download:** `inspector_fixtures.zip` (provided separately in chat; includes README).  
Suggested path in repo: `docs/fixtures/inspector/`.

---

## 10. How this bridges back to Astronomy later
The inspector is a **projection** over the same snapshot model. When you return to Astronomy:
- Inspector remains the authoritative explainable view (audit-friendly).
- Astronomy becomes another projection (spatial/LOD), fed by the same snapshot.

---

## 11. Immediate next steps
1) Add fixtures to repo and implement Phase 0 loader + validator.
2) Render tree/table/details, `$ref` navigation + backstack.
3) Wire search (by node_id and entity_id).
4) Add render policy toggles (LOD, max_depth, filters) for “zoom/prune”.

---

*End.*
