# Observatory Implementation Plan

> **Version:** 2.0 — 2026-04-04
> **Spec:** `THE_OBSERVATORY_v1.0.md`
> **Author:** Claude (planning session, no code)
> **Rendering:** egui/eframe → WASM, full-window separate browser tab
> **Revision:** v2.0 — revised from React-shell to full-egui (Option A)

---

## Architecture Decision: Full egui Application

The Observatory renders as a **standalone egui application** compiled to WASM, served in a separate browser tab at `/observatory/:sessionId`. It does NOT embed inside the React chat UI.

**Rationale:**
- egui's immediate mode IS the dumb-client invariant: every frame, paint the current struct
- No React reconciliation layer between server data and pixels
- One event loop, one coordinate system, one invalidation boundary
- Camera, panels, canvas, and transitions share the same frame loop
- Communication via server (both tabs talk to same session), not inter-tab messaging

**Boundary:**

| Surface | Technology | Why |
|---|---|---|
| Chat transcript + sidebar | React (existing) | Text-heavy, DOM-optimal |
| Observatory workspace | egui/eframe WASM (new) | 60fps, same Rust types, no translation layer |

**Communication:** Both tabs share the session ID. Both call the same REST API. Server is the synchronization point. No postMessage, no SharedWorker.

---

## Identity Verification Report

### Traceability Matrix

Every OrientationContract field traced to existing source types:

| OrientationContract field | Source type | Source file | Mapping |
|---|---|---|---|
| `session_mode` | `AgentMode` | `sem_os_core/src/authoring/agent_mode.rs` | **Direct** — Research, Governed, Maintenance |
| `view_level` | `ViewLevel` | `ob-poc-types/src/galaxy.rs` | **Direct** — 6-variant enum |
| `focus_kind` | `SubjectRef` + `ObjectRef.object_type` | `context_resolution.rs`, `stewardship/types.rs` | **Projection** |
| `focus_identity` | `SubjectRef::id()` + entity name | `context_resolution.rs`, `constellation_runtime.rs` | **Projection** |
| `scope` | `NavigationScope` | `ob-poc-types/src/galaxy.rs` | **Projection** |
| `lens.overlay` | `OverlayMode` | `stewardship/types.rs` | **Direct** |
| `lens.depth_probe` | `DepthType` | `ob-poc-types/src/galaxy.rs` | **Direct** |
| `lens.cluster_mode` | `ClusterType` | `ob-poc-types/src/galaxy.rs` | **Direct** |
| `available_actions` | `ContextResolutionResponse.candidate_verbs` + `GroundedActionSurface.valid_actions` | `context_resolution.rs` | **Direct** |
| `entry_reason` | **NOVEL** | — | 6-variant enum, captures navigation cause |
| `delta_from_previous` | **NOVEL** (computed) | — | Pure diff function |

### Gap Report

```
EXISTING (direct mapping):         9 fields
EXISTING (needs projection):       4 fields
NOVEL (new field on existing):     1 (AgentMode::Maintenance)
NOVEL (new type required):         3 (OrientationContract, EntryReason, OrientationDelta)
```

### GraphSceneModel Derivability

All 6 fields (nodes, edges, groups, drill_targets, layout_strategy, depth_encoding) fully derivable from `HydratedConstellation` + `HydratedSlot`. See v1.0 plan for full trace.

---

## What's Built (Phase 1 Rust — all VALID)

These carry forward unchanged regardless of rendering choice:

| File | Purpose |
|---|---|
| `sem_os_core/src/observatory/orientation.rs` | OrientationContract, ViewLevel, FocusKind, LensState, EntryReason, OrientationDelta (8 tests) |
| `sem_os_core/src/observatory/projection.rs` | `project_orientation()`, `compute_delta()` (4 tests) |
| `sem_os_core/src/authoring/agent_mode.rs` | AgentMode::Maintenance variant |
| `stewardship/types.rs` | ShowPacket.orientation field |
| `api/observatory_routes.rs` | REST endpoints (orientation, show-packet, navigation-history, health) |

## What's Superseded (Phase 1–2 React — replaced by egui)

These React components will be removed. Their functionality moves into the egui application:

| File | Replacement |
|---|---|
| `features/observatory/LocationHeader.tsx` | egui TopBottomPanel |
| `features/observatory/TransitionNotice.tsx` | egui transition overlay |
| `features/observatory/Breadcrumbs.tsx` | egui breadcrumb bar |
| `features/observatory/ViewportRenderer.tsx` | egui viewport dispatcher |
| `features/observatory/viewports/FocusCards.tsx` | egui Focus viewport |
| `features/observatory/viewports/ObjectTable.tsx` | egui Object viewport |
| `features/observatory/viewports/DiffView.tsx` | egui Diff viewport |
| `features/observatory/viewports/GatesPanel.tsx` | egui Gates viewport |
| `features/observatory/ObservatoryPane.tsx` | egui app root |
| `features/observatory/MissionControl.tsx` | egui maintenance panel |
| `features/observatory/HealthPanel.tsx` | egui health panel |
| `features/observatory/MaintenanceTimeline.tsx` | egui timeline panel |
| `types/observatory.ts` | Not needed (Rust types consumed directly) |
| `api/observatory.ts` | Not needed (Rust HTTP client in WASM) |
| `stores/observatory.ts` | Not needed (egui owns all state) |

**ChatPage.tsx** reverts to a simple "Open Observatory" button that opens `/observatory/:sessionId` in a new tab.

---

## Revised Phase Plan

### Phase 1 (COMPLETE — Rust backend)

Rust types and API endpoints are done. All carry forward.
- OrientationContract + projection functions
- ShowPacket integration
- REST API routes
- AgentMode::Maintenance

**Remaining Phase 1 work:**
- Remove superseded React observatory components
- Simplify ChatPage to "Open Observatory" button
- Add `/observatory/:sessionId` route in React that serves the WASM app

### Phase 2: egui Crate Bootstrap + Shell

**New crate: `observatory-wasm/`** (separate from `rust/`, at repo root like `bpmn-lite/`)

- Cargo.toml targeting `wasm32-unknown-unknown`
- Dependencies: `ob-poc-types`, `egui`, `eframe` (web), `wasm-bindgen`, `serde`, `serde_json`, `reqwest` (WASM feature)
- Does NOT depend on `sem_os_core` (avoids tokio/prost WASM blockers)
- eframe::WebRunner mounting on document.body
- HTTP client fetching OrientationContract + ShowPacket from REST API
- Shell: TopBottomPanel (Location Header), SidePanel (viewports), CentralPanel (canvas placeholder)
- Session ID from URL path

**Deliverables:**
- Empty egui app that loads in browser, fetches orientation, renders Location Header
- `wasm-pack build` producing deployable WASM
- React route `/observatory/:sessionId` serving the WASM HTML

### Phase 3: Ground Instruments (egui panels)

Rebuild Phase 1 viewports as egui panels:
- FocusCards panel (egui cards from ShowPacket focus viewport)
- ObjectTable panel (egui table from ShowPacket object viewport)
- DiffView panel (egui diff display)
- GatesPanel (egui guardrail cards with severity coloring)
- ViewportRenderer dispatcher (match on ViewportKind)
- Breadcrumbs bar
- TransitionNotice overlay

**SIGN-OFF:** All viewport data sourced from existing ShowPacket types via REST. No new queries.

### Phase 4: Mission Control (egui)

- HealthPanel (6 metrics from GET /api/observatory/health)
- MaintenanceTimeline (session lifecycle entries)
- Quick-action verb buttons
- Tab switching: Observe / Mission Control

**SIGN-OFF:** Maintenance results identical from REPL and Observatory.

### Phase 5: GraphSceneModel + Constellation Canvas

- `GraphSceneModel` type in `ob-poc-types/src/graph_scene.rs`
- Projection function in `sem_os_core/src/observatory/graph_scene_projection.rs`
- REST endpoint: GET /api/observatory/session/:id/graph-scene
- egui CentralPanel: painter-driven constellation renderer
- System-level proof of concept (deterministic orbital layout)
- Interaction: click → drill request → server → new orientation + scene
- Camera: zoom, pan (observation frame only, no semantic effect)

**SIGN-OFF:** GraphSceneModel is projection of HydratedConstellation. Drill round-trips through server.

### Phase 6: Full Level Renderers

- Universe: force-directed cluster layout (Fruchterman-Reingold in Rust)
- Cluster: force within fixed boundary
- System: deterministic orbital (extended from Phase 5)
- Planet: relationship graph with hierarchical hints
- Surface: NOT canvas — egui structured panels (attribute table, state machine, verb palette)
- Core: tree/DAG layout for ownership/control chains
- ViewTransition animations using galaxy.rs CameraPath
- Depth-encoded backgrounds using DepthColors

**SIGN-OFF:** Full drill through all 6 levels produces same GroundedActionSurface as REPL.

### Phase 7: Navigation + Observation Controls

- Navigation verbs (nav.drill, nav.zoom-out, etc.) in SemOS registry
- Agent suggestions UI (egui chips with confidence)
- History controls (back/forward replaying OrientationContracts)
- Full observation frame: anchor, focus-lock, magnified inset, minimap
- Semantic drill vs observational movement enforced

**SIGN-OFF:** Nav verbs produce same workspace stack transitions as REPL.

### Phase 8: Phase 2 Viewports + Star Charts

- TaxonomyTree, ImpactGraph, ActionSurface, CoverageMap viewports (egui)
- MermaidPanel (render Mermaid SVG strings from server)
- ConstellationMapView (static map from ConstellationMapDef)
- Overlay mode toggle, keyboard shortcuts

**SIGN-OFF:** All 8 viewports consume existing server types. No novel data.

---

## Architectural Decisions

### Deployment
- Separate browser tab at `/observatory/:sessionId`
- eframe::WebRunner targets document.body (full viewport)
- WASM built via `wasm-pack build --target web`
- Served by the same Rust server as static assets

### Shared Types
- `GraphSceneModel` lives in `ob-poc-types` (WASM-safe, no sem_os_core dep)
- Projection function lives in `sem_os_core` (server-only, needs HydratedConstellation)
- egui app depends on `ob-poc-types` only

### Communication
- egui WASM app calls REST API (same endpoints React uses)
- `reqwest` with WASM feature for HTTP in browser
- Session ID from URL, polling or SSE for live updates

### Build Pipeline
- `cd observatory-wasm && wasm-pack build --target web`
- Output: `observatory-wasm/pkg/` with .wasm + JS glue
- Served from React's dist or a dedicated static path

---

## Risk Register

| Risk | Severity | Mitigation |
|---|---|---|
| egui panel/chrome quality vs React | Medium | egui's built-in widgets handle tables, cards, grids well. Custom painting for anything beyond. |
| WASM module load time | Low | Lazy-load, ~200KB gzipped. Show loading indicator. |
| Force layout convergence | Low | Fixed iteration count, deterministic seed |
| eframe web backend maturity | Low | eframe 0.31+ stable on web, Rust 1.94 compatible |
| reqwest WASM HTTP client | Low | Well-tested WASM target, used in production elsewhere |
| No hot reload for egui dev | Medium | `cargo watch` + wasm-pack rebuild. Slower than React HMR. |

---

*PLAN v2.0 COMPLETE — Rust backend carries forward, React observatory components superseded by egui, separate browser tab deployment.*
