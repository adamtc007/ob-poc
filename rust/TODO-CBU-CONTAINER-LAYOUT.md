# TODO: CBU Container Layout Specification

## Overview

Complete specification for CBU container visualization with:
- **CBU Container** - Visual boundary containing entities
- **Entities Inside** - Trading network with roles/edges
- **External Attachments** - Instrument Matrix + Product Matrix outside container

---

## Visual Architecture

```
                              VIEWPORT
    ┌─────────────────────────────────────────────────────────────────────┐
    │                                                                     │
    │   ┌─────────────────────────────────────────────────────────────┐  │
    │   │                    📊 INSTRUMENT MATRIX                      │  │
    │   │                    (TradingMatrix)                          │  │
    │   │   ┌──────────┐  ┌──────────┐  ┌──────────┐                 │  │
    │   │   │ Equity   │  │ FI       │  │ FX       │  ← Market       │  │
    │   │   │ ┌──────┐ │  │ ┌──────┐ │  │ ┌──────┐ │                 │  │
    │   │   │ │Custdy│ │  │ │Custdy│ │  │ │Custdy│ │  ← Custody Type │  │
    │   │   │ └──────┘ │  │ └──────┘ │  │ └──────┘ │                 │  │
    │   │   └──────────┘  └──────────┘  └──────────┘                 │  │
    │   └──────────────────────────┬──────────────────────────────────┘  │
    │                              │ attachment edge                      │
    │   ┌───────────────────────────────────────────────────────────────┐│
    │   │                      CBU CONTAINER                            ││
    │   │  ┌─────────────────────────────────────────────────────────┐ ││
    │   │  │                                                         │ ││
    │   │  │      👤 ──owns 45%──→ 🏢 ──controls──→ 🏛               │ ││
    │   │  │   Investor          ManCo            Fund                │ ││
    │   │  │   (UBO)             (IM role)        (Issuer)           │ ││
    │   │  │                                                         │ ││
    │   │  │      👤 ──owns 30%──→ 🏢                                │ ││
    │   │  │   Investor          Holding Co                          │ ││
    │   │  │                                                         │ ││
    │   │  │   ════════════════════════════════════════════════     │ ││
    │   │  │   Role Legend: ● UBO  ● IM  ● Custodian  ● TA          │ ││
    │   │  │                                                         │ ││
    │   │  └─────────────────────────────────────────────────────────┘ ││
    │   │                         CBU: "Apex Capital Fund I"           ││
    │   │                         Status: 🟢 Active                     ││
    │   └───────────────────────────┬───────────────────────────────────┘│
    │                               │ attachment edge                     │
    │   ┌────────────────────────────────────────────────────────────┐   │
    │   │                    📦 PRODUCT MATRIX                        │   │
    │   │                    (ServiceTaxonomy)                        │   │
    │   │                                                             │   │
    │   │   Product: CUSTODY           Product: FUND_ADMIN            │   │
    │   │   ├─ Service: SAFEKEEPING    ├─ Service: NAV_CALC           │   │
    │   │   │  ├─ Intent: ✅           │  ├─ Intent: ✅               │   │
    │   │   │  └─ Resources: 3/3       │  └─ Resources: 2/4           │   │
    │   │   └─ Service: SETTLEMENT     └─ Service: REPORTING          │   │
    │   │      ├─ Intent: ✅              ├─ Intent: ⏳                │   │
    │   │      └─ Resources: 5/5          └─ Resources: 0/3           │   │
    │   │                                                             │   │
    │   └────────────────────────────────────────────────────────────┘   │
    │                                                                     │
    └─────────────────────────────────────────────────────────────────────┘
```

---

## Node Types & Container Membership

### Container Node (CBU)
```rust
GraphNodeData {
    id: cbu_id,
    node_type: "cbu",
    label: "Apex Capital Fund I",
    is_container: true,
    contains_type: Some("entity"),
    container_parent_id: None,  // CBU is root container
    // ... position, style
}
```

### Interior Nodes (Entities with Trading Roles)
```rust
GraphNodeData {
    id: entity_id,
    node_type: "entity",
    label: "John Smith",
    is_container: false,
    contains_type: None,
    container_parent_id: Some(cbu_id),  // INSIDE the CBU container
    metadata: {
        "roles": ["ubo", "investor"],
        "entity_type": "person",
        "jurisdiction": "US",
    }
}
```

### Exterior Nodes (Attached Taxonomies)
```rust
// Instrument Matrix root
GraphNodeData {
    id: "instrument-matrix-root",
    node_type: "matrix_root",
    label: "Instrument Matrix",
    is_container: true,           // It's a container for market nodes
    contains_type: Some("market"),
    container_parent_id: None,    // NOT inside CBU - attached outside
}

// Product Matrix root  
GraphNodeData {
    id: "product-matrix-root",
    node_type: "taxonomy_root",
    label: "Product Matrix",
    is_container: true,
    contains_type: Some("product"),
    container_parent_id: None,    // NOT inside CBU - attached outside
}
```

---

## Edge Types

### Interior Edges (Inside Container)
Edges between entities inside the CBU container:

| Edge Type | From | To | Label | Style |
|-----------|------|-----|-------|-------|
| `owns` | Person/Corp | Corp/Fund | "45%" | Solid, with percentage |
| `controls` | Corp | Corp/Fund | - | Dashed |
| `has_role` | Entity | Role | role_type | Dotted |
| `manages` | ManCo | Fund | - | Solid |

### Attachment Edges (Container to External)
Edges connecting CBU container to external taxonomies:

| Edge Type | From | To | Purpose |
|-----------|------|-----|---------|
| `has_trading_config` | CBU | Instrument Matrix Root | Links to trading setup |
| `has_products` | CBU | Product Matrix Root | Links to service config |

### Hierarchy Edges (Inside External Taxonomies)
Edges within the taxonomy trees:

**Instrument Matrix:**
```
Matrix Root ──contains──→ Market ──contains──→ CustodyType ──contains──→ Config
```

**Product Matrix:**
```
Taxonomy Root ──contains──→ Product ──contains──→ Service ──contains──→ Resource
```

---

## Layout Algorithm

### Phase 1: Identify Containers
```rust
fn identify_containers(nodes: &[GraphNodeData]) -> Vec<ContainerInfo> {
    nodes.iter()
        .filter(|n| n.is_container)
        .map(|n| ContainerInfo {
            id: n.id.clone(),
            contains_type: n.contains_type.clone(),
            children: find_children(nodes, &n.id),
        })
        .collect()
}

fn find_children(nodes: &[GraphNodeData], container_id: &str) -> Vec<String> {
    nodes.iter()
        .filter(|n| n.container_parent_id.as_deref() == Some(container_id))
        .map(|n| n.id.clone())
        .collect()
}
```

### Phase 2: Layout Interior Nodes
For nodes inside CBU container, use force-directed or hierarchical layout:

```rust
fn layout_container_interior(
    container: &ContainerInfo,
    nodes: &mut [GraphNodeData],
    edges: &[GraphEdgeData],
) {
    let interior_nodes: Vec<_> = nodes.iter_mut()
        .filter(|n| n.container_parent_id.as_deref() == Some(&container.id))
        .collect();
    
    // Option 1: Force-directed for ownership networks
    if container.contains_type == Some("entity") {
        force_directed_layout(interior_nodes, edges);
    }
    
    // Option 2: Hierarchical for trees
    else {
        hierarchical_layout(interior_nodes, edges);
    }
}
```

### Phase 3: Compute Container Bounds
After interior layout, compute container bounding box:

```rust
fn compute_container_bounds(
    container_id: &str,
    nodes: &[GraphNodeData],
    padding: f32,
) -> Rect {
    let children: Vec<_> = nodes.iter()
        .filter(|n| n.container_parent_id.as_deref() == Some(container_id))
        .collect();
    
    if children.is_empty() {
        return default_container_rect();
    }
    
    let min_x = children.iter().map(|n| n.position.x - n.radius).min();
    let max_x = children.iter().map(|n| n.position.x + n.radius).max();
    let min_y = children.iter().map(|n| n.position.y - n.radius).min();
    let max_y = children.iter().map(|n| n.position.y + n.radius).max();
    
    Rect {
        min: Pos2::new(min_x - padding, min_y - padding),
        max: Pos2::new(max_x + padding, max_y + padding),
    }
}
```

### Phase 4: Position External Attachments
Position Instrument Matrix above, Product Matrix below:

```rust
fn position_external_attachments(
    cbu_bounds: Rect,
    instrument_matrix: &mut GraphNodeData,
    product_matrix: &mut GraphNodeData,
    gap: f32,
) {
    // Instrument Matrix: centered above CBU
    instrument_matrix.position = Pos2::new(
        cbu_bounds.center().x,
        cbu_bounds.min.y - gap - instrument_matrix_height / 2.0,
    );
    
    // Product Matrix: centered below CBU
    product_matrix.position = Pos2::new(
        cbu_bounds.center().x,
        cbu_bounds.max.y + gap + product_matrix_height / 2.0,
    );
}
```

---

## Rendering Order

```rust
fn render_cbu_viewport(
    painter: &Painter,
    graph: &CbuGraphData,
    state: &CbuGraphWidget,
) {
    // 1. Render container backgrounds (bottom layer)
    render_container_backgrounds(painter, graph);
    
    // 2. Render edges inside containers
    render_interior_edges(painter, graph);
    
    // 3. Render attachment edges (CBU to external)
    render_attachment_edges(painter, graph);
    
    // 4. Render nodes inside containers
    render_interior_nodes(painter, graph, state);
    
    // 5. Render external taxonomy nodes
    render_external_taxonomies(painter, graph, state);
    
    // 6. Render container borders (top layer for containers)
    render_container_borders(painter, graph);
    
    // 7. Render labels and badges
    render_labels(painter, graph);
}
```

---

## Container Rendering

### CBU Container Box
```rust
fn render_cbu_container(
    painter: &Painter,
    cbu: &GraphNodeData,
    bounds: Rect,
    is_selected: bool,
) {
    // Background fill
    let fill = if is_selected {
        Color32::from_rgba_unmultiplied(59, 130, 246, 20)  // blue-500 @ 8%
    } else {
        Color32::from_rgba_unmultiplied(30, 41, 59, 40)    // slate-800 @ 16%
    };
    painter.rect_filled(bounds, 12.0, fill);
    
    // Border
    let stroke = if is_selected {
        Stroke::new(2.0, Color32::from_rgb(59, 130, 246))  // blue-500
    } else {
        Stroke::new(1.0, Color32::from_rgb(71, 85, 105))   // slate-600
    };
    painter.rect_stroke(bounds, 12.0, stroke);
    
    // Header bar
    let header_rect = Rect::from_min_size(
        bounds.min,
        Vec2::new(bounds.width(), 32.0),
    );
    painter.rect_filled(
        header_rect,
        Rounding { nw: 12.0, ne: 12.0, sw: 0.0, se: 0.0 },
        Color32::from_rgb(30, 41, 59),  // slate-800
    );
    
    // CBU name in header
    painter.text(
        header_rect.center(),
        Align2::CENTER_CENTER,
        &cbu.label,
        FontId::proportional(14.0),
        Color32::WHITE,
    );
    
    // Status badge
    let status_color = match cbu.metadata.get("status").and_then(|v| v.as_str()) {
        Some("active") => Color32::from_rgb(34, 197, 94),   // green-500
        Some("pending") => Color32::from_rgb(250, 204, 21), // yellow-400
        Some("blocked") => Color32::from_rgb(239, 68, 68),  // red-500
        _ => Color32::GRAY,
    };
    painter.circle_filled(
        Pos2::new(bounds.max.x - 20.0, header_rect.center().y),
        6.0,
        status_color,
    );
}
```

### External Taxonomy Containers
```rust
fn render_taxonomy_container(
    painter: &Painter,
    root: &GraphNodeData,
    bounds: Rect,
    icon: &str,  // "📊" or "📦"
) {
    // Subtle background
    painter.rect_filled(
        bounds,
        8.0,
        Color32::from_rgba_unmultiplied(15, 23, 42, 60),  // slate-900 @ 24%
    );
    
    // Dashed border (indicates "attached" not "contained")
    // Note: egui doesn't have dashed lines natively, use segments or solid
    painter.rect_stroke(
        bounds,
        8.0,
        Stroke::new(1.0, Color32::from_rgb(100, 116, 139)),  // slate-500
    );
    
    // Icon + title
    painter.text(
        Pos2::new(bounds.min.x + 12.0, bounds.min.y + 16.0),
        Align2::LEFT_CENTER,
        format!("{} {}", icon, root.label),
        FontId::proportional(12.0),
        Color32::from_rgb(148, 163, 184),  // slate-400
    );
}
```

---

## Attachment Edge Rendering

```rust
fn render_attachment_edge(
    painter: &Painter,
    from_bounds: Rect,   // CBU container bounds
    to_bounds: Rect,     // External taxonomy bounds
    attachment_type: AttachmentType,
) {
    let (start, end) = match attachment_type {
        AttachmentType::Above => (
            Pos2::new(from_bounds.center().x, from_bounds.min.y),
            Pos2::new(to_bounds.center().x, to_bounds.max.y),
        ),
        AttachmentType::Below => (
            Pos2::new(from_bounds.center().x, from_bounds.max.y),
            Pos2::new(to_bounds.center().x, to_bounds.min.y),
        ),
        AttachmentType::Left => (
            Pos2::new(from_bounds.min.x, from_bounds.center().y),
            Pos2::new(to_bounds.max.x, to_bounds.center().y),
        ),
        AttachmentType::Right => (
            Pos2::new(from_bounds.max.x, from_bounds.center().y),
            Pos2::new(to_bounds.min.x, to_bounds.center().y),
        ),
    };
    
    // Draw connector line
    painter.line_segment(
        [start, end],
        Stroke::new(1.5, Color32::from_rgb(100, 116, 139)),  // slate-500
    );
    
    // Draw attachment point circles
    painter.circle_filled(start, 4.0, Color32::from_rgb(100, 116, 139));
    painter.circle_filled(end, 4.0, Color32::from_rgb(100, 116, 139));
}

enum AttachmentType {
    Above,  // Instrument Matrix
    Below,  // Product Matrix
    Left,
    Right,
}
```

---

## Zoom/Drill Navigation

When user clicks on external taxonomy, transition to detail view:

### Zoom Into Instrument Matrix
```rust
fn handle_instrument_matrix_click(state: &mut AppState) {
    // 1. Set view mode to InstrumentMatrix
    state.panels.browser_tab = BrowserTab::TradingMatrix;
    
    // 2. Animate zoom transition
    state.graph_widget.animate_zoom_to_rect(instrument_matrix_bounds);
    
    // 3. Show trading matrix browser panel
    state.trading_matrix_state.expand_first_level(&state.trading_matrix);
}
```

### Zoom Into Product Matrix
```rust
fn handle_product_matrix_click(state: &mut AppState) {
    // 1. Set view mode to ProductMatrix
    state.panels.browser_tab = BrowserTab::ServiceResources;
    
    // 2. Animate zoom transition  
    state.graph_widget.animate_zoom_to_rect(product_matrix_bounds);
    
    // 3. Show service taxonomy browser panel
    state.service_taxonomy_state.expand_to_depth(&state.service_taxonomy, 1);
}
```

### Drill Into Hierarchy
```rust
// Product → Service → Resource drill-down
fn handle_taxonomy_node_click(
    state: &mut AppState,
    node_id: &str,
    node_type: &str,
) {
    match node_type {
        "product" => {
            // Expand to show services
            state.service_taxonomy_state.toggle_expand(node_id);
        }
        "service" => {
            // Expand to show intents/resources
            state.service_taxonomy_state.toggle_expand(node_id);
        }
        "resource" => {
            // Show resource detail panel
            state.selected_resource_id = Some(node_id.to_string());
            state.panels.show_entity_detail = true;
        }
        _ => {}
    }
}
```

---

## Data Flow

### Server → UI Data Path

```
Server (config_driven_builder.rs)
    ↓ builds
LegacyGraphNode {
    is_container: true/false,
    contains_type: Some("entity"),
    container_parent_id: Some(cbu_id),
}
    ↓ serializes to
GraphNode (ob-poc-types)
    ↓ API response
/api/cbu/:id/graph
    ↓ deserializes to
GraphNodeData (ob-poc-graph)
    ↓ layout engine
LayoutGraph with container grouping
    ↓ renders
egui Painter
```

### Required Server Data

**For Entity Graph (inside container):**
- `GET /api/cbu/:id/graph` → entities with roles, ownership edges

**For Instrument Matrix (attached outside):**
- `GET /api/cbu/:id/trading-matrix` → TradingMatrix tree ✅ EXISTS

**For Product Matrix (attached outside):**
- `GET /api/cbu/:id/service-taxonomy` → ServiceTaxonomy tree ❌ TODO

---

## Implementation Checklist

### Container Infrastructure
- [x] `is_container` field on GraphNodeData
- [x] `contains_type` field on GraphNodeData  
- [x] `container_parent_id` field on GraphNodeData
- [x] Server sets container fields in config_driven_builder.rs
- [ ] Layout engine groups nodes by container_parent_id
- [ ] Compute container bounds from children
- [ ] Render container backgrounds before nodes
- [ ] Render container borders after nodes

### CBU Container Rendering
- [ ] Header bar with CBU name
- [ ] Status badge (green/yellow/red)
- [ ] Rounded container border
- [ ] Selection highlight state
- [ ] Hover state with tooltip

### External Taxonomy Attachment
- [ ] Position Instrument Matrix above CBU
- [ ] Position Product Matrix below CBU
- [ ] Render attachment edges (dashed/connector style)
- [ ] Attachment point indicators (circles)
- [ ] Click handlers for zoom-into-taxonomy

### Interior Layout (Entities)
- [ ] Force-directed layout for ownership network
- [ ] Edge routing inside container
- [ ] Role-based node coloring
- [ ] Ownership percentage labels on edges
- [ ] UBO highlight/badge

### Zoom Navigation
- [ ] Click Instrument Matrix → zoom to trading view
- [ ] Click Product Matrix → zoom to service view
- [ ] Drill into Product → Service → Resource
- [ ] Back/breadcrumb navigation
- [ ] Smooth zoom animation

---

## File References

| File | Purpose |
|------|---------|
| `rust/src/graph/config_driven_builder.rs` | Server-side node building |
| `rust/src/graph/types.rs` | LegacyGraphNode struct |
| `rust/crates/ob-poc-types/src/lib.rs` | Shared GraphNode type |
| `rust/crates/ob-poc-graph/src/graph/types.rs` | UI-side GraphNodeData |
| `rust/crates/ob-poc-graph/src/graph/layout.rs` | Layout engine |
| `rust/crates/ob-poc-graph/src/graph/render.rs` | Rendering functions |
| `rust/crates/ob-poc-graph/src/graph/widget.rs` | CbuGraphWidget |
| `rust/crates/ob-poc-ui/src/panels/trading_matrix_browser.rs` | Instrument Matrix UI |
| `rust/crates/ob-poc-ui/src/panels/service_taxonomy.rs` | Product Matrix UI |
