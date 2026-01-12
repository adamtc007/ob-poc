# Phase 7: Viewport Scaling

**Estimated time:** ~2.5h  
**Status:** TODO  
**Dependencies:** Phase 6 (Session Refactor) ✅

---

## Overview

Light phase focused on making the force simulation and LOD system responsive to viewport size changes. Currently the simulation boundary and LOD thresholds are hardcoded - they should scale with available screen real estate.

---

## Steps

### Step 1: Force Sim Boundary Scaling (~45min)

Make the force simulation boundary dynamic based on viewport dimensions.

**Current state:**
- Hardcoded simulation bounds (e.g., 800x600 or similar)
- Nodes can cluster or spread regardless of actual viewport size

**Target:**
- Simulation boundary = f(viewport_width, viewport_height)
- Maintain aspect ratio awareness
- Padding/margin from edges

**Files:**
- `rust/crates/ob-poc-graph/src/force_layout.rs` (or equivalent)
- Look for `bounds`, `width`, `height`, `simulation_rect`

### Step 2: LOD Density-Aware Thresholds (~1h)

More viewport space = can show more detail before switching LOD levels.

**Current state:**
- Fixed LOD thresholds (e.g., >100 nodes = aggregate)
- Doesn't consider available pixels

**Target:**
- LOD thresholds scale with viewport area
- Formula: `effective_threshold = base_threshold * (viewport_area / reference_area)`
- Small viewport = aggressive aggregation
- Large viewport = more individual nodes visible

**Files:**
- `rust/crates/ob-poc-graph/src/lod.rs` (or wherever LOD logic lives)
- Look for threshold constants

### Step 3: Resize Handler Wiring (~45min)

Connect viewport resize events to the scaling logic.

**Current state:**
- Resize may not trigger recalculation
- Or recalculates but doesn't update bounds/thresholds

**Target:**
- `on_resize(new_width, new_height)` updates:
  1. Force simulation boundary
  2. LOD thresholds
  3. Triggers re-layout if needed (debounced)

**Files:**
- `rust/crates/ob-poc-ui/src/app.rs` (main update loop)
- Look for `resize`, `viewport`, `available_rect`

---

## Acceptance Criteria

- [ ] Resizing browser window scales force simulation bounds
- [ ] Small viewport triggers more aggressive LOD aggregation
- [ ] Large viewport shows more detail before aggregating
- [ ] No jitter/flicker during resize (debounce)
- [ ] Build passes: `cargo build --features server`

---

## Notes

- This is a "polish" phase - no new features, just better responsiveness
- Should be testable by resizing the browser window and observing graph behavior
- Consider caching viewport dimensions to avoid redundant recalculations

---

## Roadmap Context

| Phase | Focus | Hours | Status |
|-------|-------|-------|--------|
| 1-3 | Core wiring | - | ✅ |
| 4 | Manual camera, LOD | ~8h | TODO |
| 5 | Auto-fit, aggregation | ~9h | TODO |
| 6 | Session refactor | ~8h | ✅ |
| 7 | Viewport scaling | ~2.5h | TODO ← **this** |
