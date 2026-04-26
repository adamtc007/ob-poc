# egui Implementation Planning Prompt — for Zed/Claude Session

> Drop this into a Zed ACP session. It references the Observatory spec and existing crate structure.

---

Plan the egui implementation for the Observatory / strat renderer.

Hard constraints:
- The Observatory renders the existing SemOS runtime model. No parallel model.
- egui is a dumb client. No semantic logic in UI.
- Keep strict separation: semantic struct / render scene / observation frame.
- Zoom/pan/magnification are observation-frame only, never semantic.
- Selection != semantic focus != drill.
- No client-side semantic navigation state machine.
- Use level-specific renderers for Universe / Cluster / System / Planet / Surface / Core.
- Use egui for shell/panels/chrome and painter-driven rendering for the central strat surface.
- Plan for caching, stable IDs, invalidation boundaries, and cheap frame work.

## Existing Crate Context

The `observatory-wasm/` crate already exists at repo root with this structure:

```
observatory-wasm/
├── Cargo.toml          (egui 0.33, glow backend, ob-poc-types dep)
├── index.html          (WASM host page)
├── src/
│   ├── lib.rs          (wasm_bindgen entry, WebRunner start)
│   ├── app.rs          (eframe::App — update loop, action processing)
│   ├── state.rs        (ObservatoryState with 3-layer separation)
│   ├── actions.rs      (ObservatoryAction enum — semantic vs observation)
│   ├── fetch.rs        (ehttp async HTTP with Arc<Mutex> mailbox)
│   ├── shell/
│   │   ├── location_header.rs   (Mode • Level • Focus • Lens • Status)
│   │   ├── breadcrumbs.rs       (Navigation trail)
│   │   └── tab_bar.rs           (Observe / Mission Control tabs)
│   ├── panels/
│   │   ├── viewport_dispatcher.rs  (Routes ViewportKind → panel)
│   │   ├── focus_cards.rs, object_table.rs, diff_view.rs, gates_panel.rs
│   │   ├── action_palette.rs       (Available actions buttons)
│   │   └── mission_control/        (Health metrics + quick actions)
│   └── canvas/
│       ├── mod.rs              (Painter-driven canvas + interaction)
│       ├── controls.rs         (Minimap, anchor, zoom indicator)
│       └── levels/
│           ├── mod.rs          (Dispatch by ViewLevel)
│           ├── universe.rs     (Force-directed cluster bubbles)
│           ├── cluster.rs      (Force within boundary)
│           ├── system.rs       (Deterministic orbital layout)
│           ├── planet.rs       (Relationship graph)
│           └── core.rs         (Tree/DAG ownership chains)
```

Key shared types (in `ob-poc-types`, WASM-safe):
- `GraphSceneModel` — nodes, edges, groups, drill_targets, layout_strategy
- `SceneNode`, `SceneEdge`, `SceneGroup`, `DrillTarget`
- `LayoutStrategy` — ForceDirected, ForceWithinBoundary, DeterministicOrbital, HierarchicalGraph, TreeDag, StructuredPanels
- `ViewLevel` — Universe, Cluster, System, Planet, Surface, Core

Key patterns already established:
- Actions returned from panels, never callbacks
- `Arc<Mutex>` mailbox for ehttp async results
- `ctx.request_repaint()` during animations
- `allocate_painter()` + `RectTransform` for world-to-screen
- Single-click = selection (local), double-click = drill (semantic)
- Camera spring interpolation in `tick_camera()` before render

## What the plan should cover

Produce:
1. Implementation objectives
2. Module/file structure refinements (building on what exists)
3. egui state model refinements
4. Rendering architecture (what's painter-driven vs panel widgets)
5. Interaction model (hover, select, anchor, focus-lock, drill, zoom, pan, minimap)
6. Cache/invalidation plan
7. Performance plan
8. Per-level renderer plan (detailed — this is the main deliverable)
9. Risks/anti-patterns
10. Slice plan (ordered implementation steps)
11. Acceptance criteria

Do not write full code. Produce an implementation plan only.
The plan should build ON the existing crate, not replace it.
