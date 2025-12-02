# CBU Entity Graph Visualization - Implementation Plan

## Overview

Implement the CBU Entity Graph Visualization per CBU_ENTITY_GRAPH_SPEC.md using pure egui with custom painting. The server provides raw graph data; the UI computes layouts based on CBU category templates.

**Key Principle**: Minimal server-side changes. Layout intelligence lives in the egui client.

## Phase 1: Database Schema Changes

### 1.1 Add cbu_category to cbus table

```sql
ALTER TABLE "ob-poc".cbus ADD COLUMN cbu_category VARCHAR(30);

-- Backfill
UPDATE "ob-poc".cbus SET cbu_category = 
  CASE 
    WHEN client_type ILIKE '%fund%' THEN 'FUND_MANDATE'
    WHEN client_type ILIKE '%trust%' THEN 'FAMILY_TRUST'
    ELSE 'CORPORATE_GROUP'
  END
WHERE cbu_category IS NULL;
```

### 1.2 Role priority view for layout

```sql
CREATE OR REPLACE VIEW "ob-poc".v_cbu_entity_with_roles AS
SELECT cbu_id, entity_id, entity_name, entity_type, jurisdiction,
  array_agg(role_name ORDER BY role_priority DESC) as roles,
  (array_agg(role_name ORDER BY role_priority DESC))[1] as primary_role
FROM (
  SELECT cer.cbu_id, cer.entity_id, e.name as entity_name,
    et.type_code as entity_type, r.name as role_name,
    CASE r.name
      WHEN 'ULTIMATE_BENEFICIAL_OWNER' THEN 100
      WHEN 'SHAREHOLDER' THEN 90
      WHEN 'MANAGEMENT_COMPANY' THEN 75
      WHEN 'DIRECTOR' THEN 70
      WHEN 'TRUSTEE' THEN 71
      WHEN 'DEPOSITARY' THEN 50
      WHEN 'INVESTOR' THEN 25
      ELSE 5
    END as role_priority,
    COALESCE(lc.jurisdiction, p.jurisdiction, t.jurisdiction) as jurisdiction
  FROM "ob-poc".cbu_entity_roles cer
  JOIN "ob-poc".entities e ON cer.entity_id = e.entity_id
  JOIN "ob-poc".entity_types et ON e.entity_type_id = et.entity_type_id
  JOIN "ob-poc".roles r ON cer.role_id = r.role_id
  LEFT JOIN "ob-poc".entity_limited_companies lc ON e.entity_id = lc.entity_id
  LEFT JOIN "ob-poc".entity_partnerships p ON e.entity_id = p.entity_id
  LEFT JOIN "ob-poc".entity_trusts t ON e.entity_id = t.entity_id
) sub
GROUP BY cbu_id, entity_id, entity_name, entity_type, jurisdiction;
```

### 1.3 Update VisualizationRepository

Add cbu_category to CbuBasicView and queries.

**Files:**
- sql/migrations/YYYYMMDD_add_cbu_category.sql
- sql/migrations/YYYYMMDD_add_entity_role_views.sql
- rust/src/database/visualization_repository.rs

## Phase 2: Server API (Minimal Changes)

Add to rust/src/graph/types.rs:
- cbu_category to CbuGraph
- roles, primary_role, jurisdiction to GraphNode

Update CbuGraphBuilder to populate from new view.

**Files:**
- rust/src/graph/types.rs
- rust/src/graph/builder.rs

## Phase 3: UI Module Structure

```
rust/crates/ob-poc-ui/src/
  graph/
    mod.rs
    data.rs       # CbuGraph, CoreEntity, overlays
    template.rs   # LUX_SICAV, CAYMAN_MASTER_FEEDER, FAMILY_TRUST
    layout.rs     # Slot assignment, positioning
    types.rs      # EntityType, RoleCode enums
  render/
    mod.rs
    node.rs       # Shapes, colors, LOD
    edge.rs       # Bezier curves, hop arcs
    investor.rs   # Collapsed/expanded groups
    focus.rs      # Focus card
    colors.rs     # Palette
  interaction/
    mod.rs
    camera.rs     # Pan, zoom
    input.rs      # Mouse, keyboard
    focus_state.rs
  widget.rs       # CbuGraphWidget
```

## Phase 4: Layout Algorithm

```rust
impl CbuGraph {
    pub fn compute_layout(&mut self) {
        let slots = self.assign_entities_to_slots();
        self.position_slotted_entities(&slots);
        self.position_ubo_chains();
        self.position_investor_groups();
        self.apply_collision_avoidance();
    }
}
```

## Phase 5: Rendering

Nodes: Circle (person), RoundedRect (company), Diamond (fund), Hexagon (trust)
Edges: Quadratic bezier, hop arcs for crossings
Investor Groups: Collapsed badge, expanded list

## Phase 6: Interaction

Camera2D with smooth interpolation. Drag=pan, scroll=zoom, click=focus.

## Phase 7: Focus Mode

FocusState with highlight set. Blur non-connected. Focus card.

## Files Summary

New (18): graph/*, render/*, interaction/*, widget.rs, 2 migrations
Modified (5): types.rs, builder.rs, visualization_repository.rs, app.rs, lib.rs
